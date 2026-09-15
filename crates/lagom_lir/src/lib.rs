//! The Lagom LIR (backend-neutral machine shape) — M0 subset.
//!
//! Spec anchor: 00 §22.3 (LIR: "backend-neutral machine shape"), §23 (the
//! Cranelift backend consumes LIR), doc 08 (the one-LIR-many-backends rule).
//!
//! The M0 shape is deliberately small — the backend contract is "one LIR
//! op = one lowering decision", never "re-derive semantics":
//!
//! - **Slots** replace MIR locals: an index into the frame (params first,
//!   then locals). The frame is an array of values, every slot always
//!   initialized (`nothing` for fall-through paths — the MIR verifier's
//!   implicit-return guarantee).
//! - **Control flow is explicit**: blocks + terminators straight from MIR's
//!   CFG (LIR does not re-derive loops or pads).
//! - **Everything runtime-shaped is precomputed**: interned string constants
//!   (addressable in the data section), numeric op discriminants, and the
//!   1-based source line of every span (traps render without touching the
//!   source).
//! - **The error model is a return flag** (13.1): a call site's `failed`
//!   boolean drives the branch to the pad; `Fail { catch: None }` becomes
//!   `ReturnFailed` — the flag propagates up the frame chain untouched.

use lagom_diagnostics::Span;
use lagom_mir::MirProgram;

// ---------------------------------------------------------------------------
// Program structure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LirProgram {
    pub functions: Vec<LirFunction>,
    /// Interned string constants, in stable order (data-section layout).
    pub strings: Vec<String>,
    /// Structures in declaration order (field registration at program start).
    pub structs: Vec<(String, Vec<String>)>,
    /// The function name invoked by the entry shim's `lagom_main`.
    pub entry: String,
}

impl LirProgram {
    /// The data-section offset of an interned string.
    pub fn string_offset(&self, id: StrId) -> usize {
        self.strings[..id.0].iter().map(|s| s.len() + 1).sum()
    }
}

/// A handle into [`LirProgram::strings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrId(pub usize);

#[derive(Debug, Clone)]
pub struct LirFunction {
    pub name: String,
    /// The interned name (probe and report strings use it).
    pub name_id: StrId,
    pub params: usize,
    /// `None` = a script/test body (returns no value).
    pub ret: Option<()>,
    pub can_fail: bool,
    pub slots: usize,
    /// Interned names of the source bindings, in slot order (LOM probes).
    pub slot_names: Vec<Option<StrId>>,
    pub blocks: Vec<LirBlock>,
    /// Attempt landing pads: `(pad block, slot receiving the message)`.
    pub catch_pads: Vec<(BlockId, SlotId)>,
}

#[derive(Debug, Clone)]
pub struct LirBlock {
    pub label: String,
    pub instrs: Vec<LirInstr>,
    pub term: LirTerm,
    /// Where a failure raised by an *instruction* in this block goes.
    pub pad_on_fail: Option<BlockId>,
}

// ---------------------------------------------------------------------------
// Operands
// ---------------------------------------------------------------------------

/// A value source: a slot read or an immediate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LirOperand {
    /// An immediate number (payload = bits as i64).
    Int(i64),
    /// An immediate decimal (payload = f64 bits).
    FloatBits(i64),
    /// An immediate boolean.
    Bool(bool),
    /// The option sentinel.
    Nothing,
    /// An immediate string constant.
    Str(StrId),
    /// A frame slot read.
    Slot(SlotId),
}

impl LirOperand {
    pub fn float_bits(d: f64) -> LirOperand {
        LirOperand::FloatBits(d.to_bits() as i64)
    }
}

/// A frame slot index (params first, then locals — the frame layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockId(pub usize);

// ---------------------------------------------------------------------------
// Instructions — one op per lowering decision
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum LirInstr {
    /// `dest = operand` (a binding copy — values deep-clone via `rt_clone`).
    Copy { dest: SlotId, value: LirOperand, span: Span },
    /// `dest = op operand` (op: 0 neg, 1 not).
    Unary { dest: SlotId, op: i64, operand: LirOperand, line: i64 },
    /// `dest = left op right` (op: the `lagom_mir::BinOp` discriminant).
    Binary { dest: SlotId, op: i64, left: LirOperand, right: LirOperand, line: i64 },
    /// `dest = callee(args…)` — a user call; the failed flag is explicit.
    Call { dest: SlotId, callee: String, args: Vec<LirOperand>, line: i64 },
    /// `say value`.
    Say { value: LirOperand, line: i64 },
    /// `dest = ask question`.
    Ask { dest: SlotId, question: LirOperand, line: i64 },
    /// `dest = Name { field: value, … }`.
    StructNew { dest: SlotId, name: StrId, fields: Vec<(StrId, LirOperand)>, line: i64 },
    /// `dest = base.field` (field as an interned name).
    FieldGet { dest: SlotId, base: LirOperand, field: StrId, line: i64 },
    /// `base.field = value` — the base slot receives the rebuilt box.
    FieldSet { base: SlotId, field: StrId, value: LirOperand, line: i64 },
    /// `dest = base[index]` — list index read or map key lookup.
    IndexGet { dest: SlotId, base: LirOperand, index: LirOperand, line: i64 },
    /// `base[index] = value`.
    IndexSet { base: SlotId, index: LirOperand, value: LirOperand, line: i64 },
    /// `dest = [a, b, …]`.
    ListNew { dest: SlotId, elements: Vec<LirOperand>, line: i64 },
    /// `dest = { k: v, … }`.
    MapNew { dest: SlotId, entries: Vec<(LirOperand, LirOperand)>, line: i64 },
    /// `dest = (a, b)`.
    PairNew { dest: SlotId, first: LirOperand, second: LirOperand, line: i64 },
    /// `dest = lit₁ val₁ … ` — interpolation built by runtime chaining.
    Format { dest: SlotId, parts: Vec<FormatPart>, line: i64 },
    /// `dest = convert value` (conv: 0 number, 1 decimal, 2 text).
    Convert { dest: SlotId, conv: i64, value: LirOperand, line: i64 },
    /// `dest = random from lo to hi`.
    Random { dest: SlotId, lo: LirOperand, hi: LirOperand, line: i64 },
    /// `dest = first of list`.
    FirstOf { dest: SlotId, list: LirOperand, line: i64 },
    /// `dest = size of value`.
    SizeOf { dest: SlotId, value: LirOperand, line: i64 },
    /// `dest = join list`.
    Join { dest: SlotId, list: LirOperand, line: i64 },
    /// `dest = op value` (op: 0 uppercase, 1 lowercase, 2 trim).
    TextOp { dest: SlotId, op: i64, value: LirOperand, line: i64 },
    /// `dest = op value` (op: 0 square root, 1 floor).
    MathOp { dest: SlotId, op: i64, value: LirOperand, line: i64 },
    /// `check that value` — a failed check crashes with G-4's comparison
    /// rendering (has_cmp/cmp_op carry the operands when the checked
    /// expression was a comparison).
    Check { value: LirOperand, has_cmp: bool, cmp_op: i64, left: LirOperand, right: LirOperand, line: i64 },

    // ----- LOM probes (§26.5): dev builds emit these; release builds drop
    // them in this very lowering (the release-identity boundary) -----
    /// Function-entry probe with argument snapshots.
    LomEntry { function: StrId, args: Vec<(StrId, LirOperand)> },
    /// Binding-record probe (`site` = byte offset for the line number).
    LomBind { name: StrId, value: LirOperand, site: i64 },
    /// Failure-record probe (before a `Fail` term / routed failure).
    LomFail { message: LirOperand },
}

#[derive(Debug, Clone)]
pub enum FormatPart {
    Lit(StrId),
    Value(LirOperand),
}

// ---------------------------------------------------------------------------
// Terminators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum LirTerm {
    Goto(BlockId),
    /// `if cond → then, else otherwise` (a non-boolean condition is a crash
    /// inside rt's helper — impossible in checked programs).
    Branch { cond: LirOperand, then: BlockId, otherwise: BlockId },
    Break(BlockId),
    Continue(BlockId),
    /// `give back value` — `Nothing` for script bodies.
    Return(LirOperand),
    /// `fail with message` with no enclosing pad: propagate the failure to
    /// the caller (the failed flag; the message stays in the fail slot).
    ReturnFailed { message: LirOperand, line: i64 },
    /// `fail with message` into this function's pad.
    FailInFrame { message: LirOperand, catch: BlockId },
    /// Unreachable in verified programs (kept for round-trips).
    Unreachable,
}

// ---------------------------------------------------------------------------
// The lowering (MIR → LIR)
// ---------------------------------------------------------------------------

/// Lower a verified MIR program to LIR. `dev` selects the LOM probe policy
/// (§26.5): dev builds keep the probes, release builds never emit them —
/// the release-identity boundary is this parameter, a compile-time choice.
pub fn lower(prog: &MirProgram, dev: bool) -> LirProgram {
    let mut inr = Interner::new();

    let structs: Vec<(String, Vec<String>)> = prog
        .structs
        .iter()
        .map(|s| (s.name.clone(), s.fields.iter().map(|(n, _)| n.clone()).collect()))
        .collect();

    // S-12/D-32: `test` blocks run on the interpreter backend (`lagom test`),
    // never in native code. LIR carries the script and functions only, so
    // `lagom build`/`lagom run` are unaffected by tests and the entry is
    // always the script body.
    let mut functions = Vec::new();
    for item in &prog.items {
        match item {
            lagom_mir::MirItem::Main(f) | lagom_mir::MirItem::Function(f) => {
                functions.push(lower_function(f, dev, &mut inr))
            }
            lagom_mir::MirItem::Test(_) => {}
        }
    }

    // The entry: the script body only. A program with no top-level
    // statements (a library of functions, or tests only) has no script to
    // run — the native binary initializes the runtime and exits 0 (tests
    // stay on the interpreter per S-12).
    let entry = prog
        .items
        .iter()
        .filter(|i| matches!(i, lagom_mir::MirItem::Main(f) if f.is_entry))
        .last()
        .or_else(|| prog.items.iter().find(|i| matches!(i, lagom_mir::MirItem::Main(_))))
        .map(|i| match i {
            lagom_mir::MirItem::Main(f) => f.name.clone(),
            _ => unreachable!("filtered above"),
        })
        .unwrap_or_default();

    LirProgram { functions, strings: inr.strings, structs, entry }
}

/// The string pool: deduplicated, stable-order, data-section ready.
#[derive(Default)]
struct Interner {
    strings: Vec<String>,
    seen: std::collections::HashMap<String, StrId>,
}

impl Interner {
    fn new() -> Interner {
        Interner::default()
    }

    fn intern(&mut self, s: &str) -> StrId {
        if let Some(id) = self.seen.get(s) {
            return *id;
        }
        let id = StrId(self.strings.len());
        self.strings.push(s.to_string());
        self.seen.insert(s.to_string(), id);
        id
    }
}

fn lower_function(f: &lagom_mir::MirFunction, dev: bool, inr: &mut Interner) -> LirFunction {
    let name_id = inr.intern(&f.name);
    let mut slot_names: Vec<Option<StrId>> = Vec::new();
    for l in &f.locals {
        let id = if l.is_source_binding() && l.name != "result" && l.name != "problem" {
            Some(inr.intern(&l.name))
        } else {
            None
        };
        slot_names.push(id);
    }
    // Pads' message locals (`result`/`problem`) report by their source name.
    for (_pad, local) in &f.catch_pads {
        let name = &f.local(*local).name;
        slot_names[local.0] = Some(inr.intern(name));
    }

    let mut blocks = Vec::new();
    for b in &f.blocks {
        let mut instrs = Vec::new();
        for i in &b.instrs {
            lower_instr(i, dev, inr, &mut instrs);
        }
        let term = match &b.term {
            lagom_mir::Term::Goto(t) => LirTerm::Goto(BlockId(t.0)),
            lagom_mir::Term::Branch { cond, then, otherwise } => LirTerm::Branch {
                cond: lower_operand(cond, inr),
                then: BlockId(then.0),
                otherwise: BlockId(otherwise.0),
            },
            lagom_mir::Term::Break { target } => LirTerm::Break(BlockId(target.0)),
            lagom_mir::Term::Continue { target } => LirTerm::Continue(BlockId(target.0)),
            lagom_mir::Term::Return { value } => LirTerm::Return(lower_operand(value, inr)),
            lagom_mir::Term::Fail { message, catch, span } => {
                let message = lower_operand(message, inr);
                let line = line_of(span);
                match catch {
                    Some(pad) => LirTerm::FailInFrame { message, catch: BlockId(pad.0) },
                    None => LirTerm::ReturnFailed { message, line },
                }
            }
            lagom_mir::Term::Unreachable => LirTerm::Unreachable,
        };
        blocks.push(LirBlock {
            label: b.label.clone(),
            instrs,
            term,
            pad_on_fail: b.pad_on_fail.map(|p| BlockId(p.0)),
        });
    }

    LirFunction {
        name: f.name.clone(),
        name_id,
        params: f.params.len(),
        ret: f.ret.clone().map(|_| ()),
        can_fail: f.can_fail,
        slots: f.locals.len(),
        slot_names,
        blocks,
        catch_pads: f.catch_pads.iter().map(|(b, l)| (BlockId(b.0), SlotId(l.0))).collect(),
    }
}

fn lower_instr(i: &lagom_mir::Instr, dev: bool, inr: &mut Interner, out: &mut Vec<LirInstr>) {
    use lagom_mir::Instr as M;
    match i {
        M::Copy { dest, value, span } => out.push(LirInstr::Copy {
            dest: SlotId(dest.0),
            value: lower_operand(value, inr),
            span: *span,
        }),
        M::Unary { dest, op, operand, span } => out.push(LirInstr::Unary {
            dest: SlotId(dest.0),
            op: match op {
                lagom_mir::UnOp::Neg => 0,
                lagom_mir::UnOp::Not => 1,
            },
            operand: lower_operand(operand, inr),
            line: line_of(span),
        }),
        M::Binary { dest, op, left, right, span } => out.push(LirInstr::Binary {
            dest: SlotId(dest.0),
            op: binop_discriminant(op),
            left: lower_operand(left, inr),
            right: lower_operand(right, inr),
            line: line_of(span),
        }),
        M::Call { dest, callee, args, span, .. } => out.push(LirInstr::Call {
            dest: SlotId(dest.0),
            callee: callee.clone(),
            args: args.iter().map(|a| lower_operand(a, inr)).collect(),
            line: line_of(span),
        }),
        M::Say { value, span } => out.push(LirInstr::Say {
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::Ask { dest, question, span } => out.push(LirInstr::Ask {
            dest: SlotId(dest.0),
            question: lower_operand(question, inr),
            line: line_of(span),
        }),
        M::StructNew { dest, name, fields, span } => out.push(LirInstr::StructNew {
            dest: SlotId(dest.0),
            name: inr.intern(name),
            fields: fields
                .iter()
                .map(|(n, v)| (inr.intern(n), lower_operand(v, inr)))
                .collect(),
            line: line_of(span),
        }),
        M::FieldGet { dest, base, field, span, .. } => out.push(LirInstr::FieldGet {
            dest: SlotId(dest.0),
            base: lower_operand(base, inr),
            field: inr.intern(field),
            line: line_of(span),
        }),
        M::FieldSet { base, field, value, span, .. } => out.push(LirInstr::FieldSet {
            base: SlotId(base.as_local().map(|l| l.0).unwrap_or(0)),
            field: inr.intern(field),
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::IndexGet { dest, base, index, span } => out.push(LirInstr::IndexGet {
            dest: SlotId(dest.0),
            base: lower_operand(base, inr),
            index: lower_operand(index, inr),
            line: line_of(span),
        }),
        M::IndexSet { base, index, value, span, .. } => out.push(LirInstr::IndexSet {
            base: SlotId(base.as_local().map(|l| l.0).unwrap_or(0)),
            index: lower_operand(index, inr),
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::ListNew { dest, elements, span } => out.push(LirInstr::ListNew {
            dest: SlotId(dest.0),
            elements: elements.iter().map(|e| lower_operand(e, inr)).collect(),
            line: line_of(span),
        }),
        M::MapNew { dest, entries, span } => out.push(LirInstr::MapNew {
            dest: SlotId(dest.0),
            entries: entries
                .iter()
                .map(|(k, v)| {
                    (
                        lower_operand(k, inr),
                        lower_operand(v, inr),
                    )
                })
                .collect(),
            line: line_of(span),
        }),
        M::PairNew { dest, first, second, span } => out.push(LirInstr::PairNew {
            dest: SlotId(dest.0),
            first: lower_operand(first, inr),
            second: lower_operand(second, inr),
            line: line_of(span),
        }),
        M::Format { dest, parts, span } => out.push(LirInstr::Format {
            dest: SlotId(dest.0),
            parts: parts
                .iter()
                .map(|p| match p {
                    lagom_mir::FormatPart::Lit(s) => FormatPart::Lit(inr.intern(s)),
                    lagom_mir::FormatPart::Value(v) => {
                        FormatPart::Value(lower_operand(v, inr))
                    }
                })
                .collect(),
            line: line_of(span),
        }),
        M::Convert { dest, conv, value, span } => out.push(LirInstr::Convert {
            dest: SlotId(dest.0),
            conv: match conv {
                lagom_mir::Conv::ToNumber => 0,
                lagom_mir::Conv::ToDecimal => 1,
                lagom_mir::Conv::ToText => 2,
            },
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::Random { dest, lo, hi, span } => out.push(LirInstr::Random {
            dest: SlotId(dest.0),
            lo: lower_operand(lo, inr),
            hi: lower_operand(hi, inr),
            line: line_of(span),
        }),
        M::FirstOf { dest, list, span } => out.push(LirInstr::FirstOf {
            dest: SlotId(dest.0),
            list: lower_operand(list, inr),
            line: line_of(span),
        }),
        M::SizeOf { dest, value, span } => out.push(LirInstr::SizeOf {
            dest: SlotId(dest.0),
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::Join { dest, list, span } => out.push(LirInstr::Join {
            dest: SlotId(dest.0),
            list: lower_operand(list, inr),
            line: line_of(span),
        }),
        M::TextOp { dest, op, value, span } => out.push(LirInstr::TextOp {
            dest: SlotId(dest.0),
            op: match op {
                lagom_mir::TextOp::Uppercase => 0,
                lagom_mir::TextOp::Lowercase => 1,
                lagom_mir::TextOp::Trim => 2,
            },
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::MathOp { dest, op, value, span } => out.push(LirInstr::MathOp {
            dest: SlotId(dest.0),
            op: match op {
                lagom_mir::MathOp::SquareRoot => 0,
                lagom_mir::MathOp::Floor => 1,
            },
            value: lower_operand(value, inr),
            line: line_of(span),
        }),
        M::Check { value, cmp, span } => out.push(LirInstr::Check {
            value: lower_operand(value, inr),
            has_cmp: cmp.is_some(),
            cmp_op: cmp.as_ref().map(|(op, _, _)| binop_discriminant(op)).unwrap_or(0),
            left: cmp
                .as_ref()
                .map(|(_, l, _)| lower_operand(l, inr))
                .unwrap_or(LirOperand::Nothing),
            right: cmp
                .as_ref()
                .map(|(_, _, r)| lower_operand(r, inr))
                .unwrap_or(LirOperand::Nothing),
            line: line_of(span),
        }),

        // ----- LOM probes: dev keeps, release drops (§26.5's boundary) -----
        M::EventFunctionEntry { function, args } if dev => out.push(LirInstr::LomEntry {
            function: inr.intern(function),
            args: args
                .iter()
                .map(|(n, v)| (inr.intern(n), lower_operand(v, inr)))
                .collect(),
        }),
        M::EventBind { name, value, site } if dev => out.push(LirInstr::LomBind {
            name: inr.intern(name),
            value: lower_operand(value, inr),
            site: site.start as i64,
        }),
        M::EventFail { message } if dev => out.push(LirInstr::LomFail {
            message: lower_operand(message, inr),
        }),
        M::EventFunctionEntry { .. } | M::EventBind { .. } | M::EventFail { .. } => {}
    }
}

fn lower_operand(op: &lagom_mir::Operand, inr: &mut Interner) -> LirOperand {
    match op {
        lagom_mir::Operand::Int(v) => LirOperand::Int(*v),
        lagom_mir::Operand::Float(v) => LirOperand::float_bits(*v),
        lagom_mir::Operand::Text(s) => LirOperand::Str(inr.intern(s)),
        lagom_mir::Operand::Bool(b) => LirOperand::Bool(*b),
        lagom_mir::Operand::Nothing => LirOperand::Nothing,
        lagom_mir::Operand::Local(id) => LirOperand::Slot(SlotId(id.0)),
    }
}

/// The `lagom_mir::BinOp` discriminant (the rt's table order).
pub fn binop_discriminant(op: &lagom_mir::BinOp) -> i64 {
    match op {
        lagom_mir::BinOp::Add => 0,
        lagom_mir::BinOp::Sub => 1,
        lagom_mir::BinOp::Mul => 2,
        lagom_mir::BinOp::Div => 3,
        lagom_mir::BinOp::DivEvenly => 4,
        lagom_mir::BinOp::Rem => 5,
        lagom_mir::BinOp::And => 6,
        lagom_mir::BinOp::Or => 7,
        lagom_mir::BinOp::Equal => 8,
        lagom_mir::BinOp::NotEqual => 9,
        lagom_mir::BinOp::Greater => 10,
        lagom_mir::BinOp::Less => 11,
        lagom_mir::BinOp::AtLeast => 12,
        lagom_mir::BinOp::AtMost => 13,
    }
}

/// The 1-based source line of a span (precomputed — the backend never sees
/// the source text).
fn line_of(span: &lagom_diagnostics::Span) -> i64 {
    // Doc 08: the driver hands the source to the pipeline once; LIR carries
    // line numbers so backends and traps stay source-free. The M0 driver
    // lowers per-function with the source position already baked into spans;
    // the line number is computed here from the *program source* the caller
    // provides through `lower_with_source`.
    line_of_with(span)
}

thread_local! {
    /// The program source for line computation (set once per compilation).
    static SOURCE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Set the source text for line computation (call before `lower`).
pub fn set_source(src: &str) {
    SOURCE.with(|s| *s.borrow_mut() = src.to_string());
}

fn line_of_with(span: &Span) -> i64 {
    SOURCE.with(|s| lagom_mir::line_of(&s.borrow(), span.start) as i64)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Source → LIR through the frozen pipeline (parse → check → HIR → MIR).
    fn to_lir(src: &str) -> LirProgram {
        let (ast, pdiags) = lagom_parser::parse(src);
        assert!(pdiags.is_empty(), "parse errors in test source: {pdiags:?}");
        let checked = lagom_sema::check(&ast, src);
        assert!(
            checked.diags.items.is_empty(),
            "sema errors in test source {src:?}: {:?}",
            checked.diags.items.iter().map(|d| (&d.code, &d.message)).collect::<Vec<_>>()
        );
        let m = lagom_mir::lower(lagom_hir::lower(checked));
        lagom_mir::verify(&m).expect("MIR must verify");
        set_source(src);
        lower(&m, true)
    }

    #[test]
    fn hello_world_lowers_to_slots_and_interned_strings() {
        let lir = to_lir("say \"Hello, world!\"");
        assert_eq!(lir.entry, "main");
        // Function names intern alongside constants (probes and reports use
        // the same pool).
        assert_eq!(lir.strings, vec!["main".to_string(), "Hello, world!".to_string()]);
        let main = &lir.functions[0];
        assert_eq!(main.name, "main");
        assert!(main.ret.is_none());
        // `say` materializes its value into a local, then Say reads it (the
        // call-shape lowering — every say goes through a slot).
        assert!(matches!(
            main.blocks[0].instrs[1],
            LirInstr::Say { value: LirOperand::Slot(_), .. }
        ));
    }

    #[test]
    fn string_interning_is_deduplicated_and_stable() {
        let lir = to_lir("say \"hi\"\nsay \"hi\"");
        // "main" + one deduplicated constant.
        assert_eq!(lir.strings.len(), 2, "the same constant interns once: {:?}", lir.strings);
        let lir = to_lir("say \"a\"\nsay \"b\"");
        assert_eq!(lir.strings, vec!["main".to_string(), "a".to_string(), "b".to_string()]);
    }

    #[test]
    fn bindings_map_to_slots_and_dev_keeps_probes() {
        // Dev builds instrument the MIR (26.5) *before* lowering, so the
        // probes arrive as real MIR instructions.
        let src = "make x equal to 5\nsay x";
        let (ast, _) = lagom_parser::parse(src);
        let checked = lagom_sema::check(&ast, src);
        let mut m = lagom_mir::lower(lagom_hir::lower(checked));
        lagom_mir::instrument(&mut m, lagom_mir::LomConfig::DEV);
        set_source(src);
        let lir = lower(&m, true);
        let main = &lir.functions[0];
        assert!(main.slot_names.iter().any(|n| n.is_some()), "source bindings carry names");
        let has_bind = main
            .blocks
            .iter()
            .any(|b| b.instrs.iter().any(|i| matches!(i, LirInstr::LomBind { .. })));
        assert!(has_bind, "dev lowering keeps EventBind probes");
    }

    #[test]
    fn release_drops_all_probes() {
        let (ast, _) = lagom_parser::parse("make x equal to 5\nsay x");
        let checked = lagom_sema::check(&ast, "make x equal to 5\nsay x");
        let m = lagom_mir::lower(lagom_hir::lower(checked));
        let instrumented = {
            let mut m = m;
            lagom_mir::instrument(&mut m, lagom_mir::LomConfig::DEV);
            m
        };
        set_source("make x equal to 5\nsay x");
        let lir = lower(&instrumented, false);
        let has_probes = lir.functions.iter().any(|f| {
            f.blocks.iter().any(|b| {
                b.instrs.iter().any(|i| {
                    matches!(i, LirInstr::LomBind { .. } | LirInstr::LomEntry { .. } | LirInstr::LomFail { .. })
                })
            })
        });
        assert!(!has_probes, "release lowering must emit zero LOM probes");
    }

    #[test]
    fn fail_with_without_pad_becomes_return_failed() {
        let lir = to_lir(
            "function risky business\n    can fail\n    fail with \"boom\"\nfunction wrapper\n    attempt risky business if it fails then\n        say problem\n    otherwise\n        say \"no\"\nwrapper",
        );
        let risky = lir.functions.iter().find(|f| f.name == "risky business").unwrap();
        let has_fail = risky.blocks.iter().any(|b| matches!(b.term, LirTerm::ReturnFailed { .. }));
        assert!(has_fail, "`fail with` at a no-pad boundary lowers to ReturnFailed");
        let wrapper = lir.functions.iter().find(|f| f.name == "wrapper").unwrap();
        assert!(
            !wrapper.catch_pads.is_empty(),
            "the attempt's pad survives into LIR"
        );
    }

    #[test]
    fn lines_are_precomputed_from_spans() {
        let lir = to_lir("say 1 divided by 0");
        let main = &lir.functions[0];
        // The division is a Binary; the `say` reads its slot after (the
        // call-shape lowering routes say's value through a local).
        match &main.blocks[0].instrs[0] {
            LirInstr::Binary { line, .. } => assert_eq!(*line, 1, "the trap line is baked in"),
            other => panic!("expected a Binary, got {other:?}"),
        }
        match &main.blocks[0].instrs[1] {
            LirInstr::Say { line, .. } => assert_eq!(*line, 1),
            other => panic!("expected a Say, got {other:?}"),
        }
        let lir = to_lir("say \"a\"\nsay 1 divided by 0");
        // Line 1: say's materialization + Say. Line 2: Binary + Say.
        match &lir.functions[0].blocks[0].instrs[2] {
            LirInstr::Binary { line, .. } => assert_eq!(*line, 2),
            other => panic!("expected a Binary, got {other:?}"),
        }
    }

    #[test]
    fn structs_register_their_field_names() {
        let lir = to_lir("structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\" and score 7\nsay name of p");
        assert_eq!(lir.structs.len(), 1);
        assert_eq!(lir.structs[0], ("player".to_string(), vec!["name".to_string(), "score".to_string()]));
    }
}
