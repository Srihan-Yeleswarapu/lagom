//! The Lagom MIR (semantic core) — M0 subset.
//!
//! Spec anchor: 00 §22.2 (the semantic core), §22.4 (verifier; debug/release
//! is *checks*, never semantics), §26.5 (the LOM instrumentation pass), doc 08
//! stage responsibilities.
//!
//! M0 scope per the implementation brief: control flow, calls, list/struct
//! ops, error propagation, say/ask — **without** M1+ forms (no coroutine
//! frames, closure environments, move/borrow markers, or ARC ops; §22.2 lists
//! them, but M0's values need none).
//!
//! The frozen form is a **CFG, not a statement tree**: every function is a
//! vector of `Block`s, each terminated by exactly one `Term`. Failure
//! propagation is explicit (13.1): `Term::Fail` names the innermost attempt's
//! landing pad; the pad's `catch_pads` entry says which local receives the
//! error message. `await`/spawn are M1 — no suspended frames yet.
//!
//! **LOM hooks (§26.5) are part of the stage contract, not a bolt-on:**
//! `instrument(&mut prog, cfg)` is *the* named pass doc 08 owns — dev builds
//! get function-entry snapshots, binding provenance, and failure events;
//! release builds are stripped and stay silent (the release-identity
//! invariant, enforced and tested here).

use lagom_diagnostics::Span;
use lagom_sema::Type;

// ---------------------------------------------------------------------------
// Program structure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MirProgram {
    pub items: Vec<MirItem>,
    /// Structures in declaration order (layout input for backends).
    pub structs: Vec<MirStruct>,
}

#[derive(Debug, Clone)]
pub struct MirStruct {
    pub name: String,
    /// (field, type) in declaration order.
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MirItem {
    /// A top-level statement batch — the driver concatenates these into the
    /// program entry (the script model, §19.1).
    Main(MirFunction),
    Function(MirFunction),
    /// A test body, zero parameters (the test runner calls each).
    Test(MirFunction),
}

/// The capability set (§22.2: capabilities are explicit on functions). M0 has
/// exactly one; `wait` arrives with M1's `can wait`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Fail,
}

#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<LocalId>,
    /// `None` = the function returns no value (script/test bodies).
    pub ret: Option<Type>,
    pub can_fail: bool,
    pub capabilities: Vec<Capability>,
    pub locals: Vec<Local>,
    pub blocks: Vec<Block>,
    /// Attempt landing pads: `(pad block, local receiving the message)`.
    pub catch_pads: Vec<(BlockId, LocalId)>,
    /// True for script bodies the driver concatenates into `main`.
    pub is_entry: bool,
    pub span: Span,
}

impl MirFunction {
    pub fn local(&self, id: LocalId) -> &Local {
        &self.locals[id.0]
    }

    pub fn local_ty(&self, id: LocalId) -> Type {
        self.locals[id.0].ty.clone()
    }

    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id.0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);

#[derive(Debug, Clone)]
pub struct Local {
    /// Source name, or a `%temp` for compiler temporaries.
    pub name: String,
    pub ty: Type,
    /// True for `make changing` bindings and loop machinery.
    pub mutable: bool,
    /// Where the binding/temporary was created (LOM provenance).
    pub span: Span,
}

impl Local {
    /// True when this local corresponds to a source binding (not `%temp`).
    pub fn is_source_binding(&self) -> bool {
        !self.name.starts_with('%')
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    /// Human-readable role — LOM names blocks in source words.
    pub label: String,
    pub instrs: Vec<Instr>,
    pub term: Term,
    /// Where a failure raised *by an instruction in this block* (a failing
    /// `Call`/`Convert`) goes: the innermost attempt's landing pad. `None`
    /// means the failure propagates out of the function (13.1). Explicit
    /// `fail with` carries its own target in `Term::Fail`.
    pub pad_on_fail: Option<BlockId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

// ---------------------------------------------------------------------------
// Values and instructions
// ---------------------------------------------------------------------------

/// An operand: an immediate or a local read. The type of `Local` lives in the
/// function's local table (`MirFunction::local_ty`); immediates self-describe.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    /// The option sentinel (8.5).
    Nothing,
    Local(LocalId),
}

impl Operand {
    /// The type of an immediate. For `Local`, consult the local table — this
    /// returns `Type::Error` as a deliberate "not self-describing" marker.
    pub fn immediate_ty(&self) -> Type {
        match self {
            Operand::Int(_) => Type::Number,
            Operand::Float(_) => Type::Decimal,
            Operand::Text(_) => Type::Text,
            Operand::Bool(_) => Type::Boolean,
            Operand::Nothing => Type::Option(Box::new(Type::Error)),
            Operand::Local(_) => Type::Error,
        }
    }

    pub fn as_local(&self) -> Option<LocalId> {
        match self {
            Operand::Local(id) => Some(*id),
            _ => None,
        }
    }
}

/// The M0 value operations. §22.2's explicit memory ops are M0-reduced to
/// named locals (`Copy`) plus structured reads/writes; lists and structs are
/// runtime-managed values at M0 (ARC ops arrive with M1's reference types).
#[derive(Debug, Clone)]
pub enum Instr {
    /// `dest = value`.
    Copy { dest: LocalId, value: Operand, span: Span },
    /// `dest = op operand` (numeric negation / boolean not).
    Unary { dest: LocalId, op: UnOp, operand: Operand, span: Span },
    /// `dest = left op right`.
    Binary { dest: LocalId, op: BinOp, left: Operand, right: Operand, span: Span },
    /// `dest = callee(args)` — a user function call.
    Call { dest: LocalId, callee: String, callee_span: Span, args: Vec<Operand>, span: Span },
    /// `say value` (S-9 formatting applies in the runtime's one printing place).
    Say { value: Operand, span: Span },
    /// `dest = ask question` (reads one line of stdin as text).
    Ask { dest: LocalId, question: Operand, span: Span },
    /// `dest = Name { field: v, … }`.
    StructNew { dest: LocalId, name: String, fields: Vec<(String, Operand)>, span: Span },
    /// `dest = base.field`.
    FieldGet { dest: LocalId, base: Operand, field: String, field_span: Span, span: Span },
    /// `base.field = value`.
    FieldSet { base: Operand, field: String, value: Operand, span: Span },
    /// `dest = base[index]` — list index read or map key lookup.
    IndexGet { dest: LocalId, base: Operand, index: Operand, span: Span },
    /// `base[index] = value` — list element store or map insert.
    IndexSet { base: Operand, index: Operand, value: Operand, span: Span },
    /// `dest = [a, b, …]`.
    ListNew { dest: LocalId, elements: Vec<Operand>, span: Span },
    /// `dest = { k: v, … }`.
    MapNew { dest: LocalId, entries: Vec<(Operand, Operand)>, span: Span },
    /// `dest = (a, b)`.
    PairNew { dest: LocalId, first: Operand, second: Operand, span: Span },
    /// `dest = "a" ++ b ++ …` — interpolation lowered here (7.7); the runtime
    /// formats each part with S-9's rules in one place.
    Format { dest: LocalId, parts: Vec<FormatPart>, span: Span },
    /// `dest = convert value` — the conversion builtins (`number from …` can
    /// fail; 13.1/D-39).
    Convert { dest: LocalId, conv: Conv, value: Operand, span: Span },
    /// `dest = random from lo to hi`.
    Random { dest: LocalId, lo: Operand, hi: Operand, span: Span },
    /// `dest = first of list` — the option-producing head query (S-13).
    FirstOf { dest: LocalId, list: Operand, span: Span },
    /// `dest = size of value`.
    SizeOf { dest: LocalId, value: Operand, span: Span },
    /// `dest = join list`.
    Join { dest: LocalId, list: Operand, span: Span },
    /// `dest = op value` — the text operations.
    TextOp { dest: LocalId, op: TextOp, value: Operand, span: Span },
    /// `dest = op value` — the `math` module (G-9).
    MathOp { dest: LocalId, op: MathOp, value: Operand, span: Span },
    /// `check that value` — a test assertion; the runtime renders the
    /// comparison on failure (docs/13 G-4, doc 09's report format). When the
    /// checked expression *is* a comparison, its operator and operands ride
    /// along so the failure can name both sides.
    Check {
        value: Operand,
        cmp: Option<(BinOp, Operand, Operand)>,
        span: Span,
    },

    // ----- LOM event probes (§26.5) — emitted only by `instrument` in dev -----
    /// Function entry with argument snapshots (provenance: the parameter's
    /// own binding site).
    EventFunctionEntry { function: String, args: Vec<(String, Operand)> },
    /// A binding record: where a value came from (26.5's value provenance).
    EventBind { name: String, value: Operand, site: Span },
    /// A failure about to propagate (the ring's payoff record).
    EventFail { message: Operand },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

/// The M0 operator set (7.3). `Div` promotes (D-10); `DivEvenly` floors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    DivEvenly,
    Rem,
    And,
    Or,
    Equal,
    NotEqual,
    Greater,
    Less,
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone)]
pub enum FormatPart {
    Lit(String),
    Value(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conv {
    ToNumber,
    ToDecimal,
    ToText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOp {
    Uppercase,
    Lowercase,
    Trim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathOp {
    SquareRoot,
    Floor,
}

// ---------------------------------------------------------------------------
// Terminators — the CFG edges
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Term {
    /// Unconditional jump.
    Goto(BlockId),
    /// `if cond → then, else otherwise`.
    Branch { cond: Operand, then: BlockId, otherwise: BlockId },
    /// `stop` — leave the enclosing loop.
    Break { target: BlockId },
    /// `next` — continue the enclosing loop.
    Continue { target: BlockId },
    /// `give back value`.
    Return { value: Operand },
    /// `fail with message` — jump to `catch` (the innermost attempt's landing
    /// pad), or out of the function when `catch` is `None` (13.1: the caller
    /// sees the failure; sema's E0302 check proves every path is caught or the
    /// function declares `can fail`).
    Fail { message: Operand, catch: Option<BlockId>, span: Span },
    /// Builder-internal marker for a block not yet terminated. The verifier
    /// rejects it in finished functions (§22.4).
    #[doc(hidden)]
    Unreachable,
}

// ---------------------------------------------------------------------------
// Construction: HIR → MIR (structural)
// ---------------------------------------------------------------------------

/// Lower a checked-and-desugared program to MIR. Structural only — run
/// [`instrument`] afterwards to add the LOM probes a build mode needs.
pub fn lower(program: lagom_hir::HirProgram) -> MirProgram {
    let mut structs: Vec<MirStruct> = Vec::new();
    let mut items: Vec<MirItem> = Vec::new();
    // Top-level statements merge into ONE script body (§19.1's script model —
    // sema already checked them against one shared top-level scope).
    let mut script: Vec<lagom_hir::HirStmt> = Vec::new();
    for item in program.items {
        match item {
            lagom_hir::HirItem::Use { .. } => {}
            lagom_hir::HirItem::Structure(s) => structs.push(MirStruct {
                name: s.name,
                fields: s.fields.iter().map(|(n, t, _)| (n.clone(), t.clone())).collect(),
                span: Span::default(),
            }),
            lagom_hir::HirItem::Function(f) => items.push(MirItem::Function(lower_function(f, false))),
            lagom_hir::HirItem::Test(f) => items.push(MirItem::Test(lower_function(f, false))),
            lagom_hir::HirItem::Main(stmt) => script.push(stmt),
        }
    }
    if !script.is_empty() {
        items.push(MirItem::Main(lower_script(script)));
    }
    MirProgram { items, structs }
}

/// The build-mode configuration — a compile-time choice, not a runtime
/// switch (§26.5: "observability is a build mode, not a runtime cost you opt
/// out of").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LomConfig {
    pub dev: bool,
}

impl LomConfig {
    pub const DEV: LomConfig = LomConfig { dev: true };
    pub const RELEASE: LomConfig = LomConfig { dev: false };
}

fn lower_function(f: lagom_hir::HirFunction, is_entry: bool) -> MirFunction {
    let mut lx = FunctionLowerer::new(f.name.clone(), Some(f.ret.clone()), f.can_fail, is_entry, f.span);
    for (name, ty, span) in &f.params {
        let id = lx.new_local(name, ty.clone(), false, *span);
        lx.declare(name, id);
        lx.params.push(id);
    }
    lx.push_block("entry");
    for s in f.body {
        lx.stmt(s);
    }
    lx.finish()
}

fn lower_script(stmts: Vec<lagom_hir::HirStmt>) -> MirFunction {
    let span = stmts.first().map(|s| s.span).unwrap_or_default();
    let mut lx = FunctionLowerer::new("main".to_string(), None, false, true, span);
    lx.push_block("entry");
    for s in stmts {
        lx.stmt(s);
    }
    lx.finish()
}

struct FunctionLowerer {
    name: String,
    ret: Option<Type>,
    can_fail: bool,
    is_entry: bool,
    span: Span,
    params: Vec<LocalId>,
    locals: Vec<Local>,
    scopes: Vec<std::collections::HashMap<String, LocalId>>,
    blocks: Vec<Block>,
    active: BlockId,
    /// Stack of open attempt pads — a `Fail` inside binds to the innermost.
    catches: Vec<BlockId>,
    /// `(join, cond)` of each enclosing loop, innermost last.
    loop_targets: Vec<(BlockId, BlockId)>,
    catch_pads: Vec<(BlockId, LocalId)>,
}

impl FunctionLowerer {
    fn new(name: String, ret: Option<Type>, can_fail: bool, is_entry: bool, span: Span) -> Self {
        FunctionLowerer {
            name,
            ret,
            can_fail,
            is_entry,
            span,
            params: Vec::new(),
            locals: Vec::new(),
            scopes: vec![std::collections::HashMap::new()],
            blocks: Vec::new(),
            active: BlockId(0),
            catches: Vec::new(),
            loop_targets: Vec::new(),
            catch_pads: Vec::new(),
        }
    }

    fn new_local(&mut self, name: &str, ty: Type, mutable: bool, span: Span) -> LocalId {
        let id = LocalId(self.locals.len());
        self.locals.push(Local { name: name.to_string(), ty, mutable, span });
        id
    }

    fn declare(&mut self, name: &str, id: LocalId) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), id);
    }

    fn resolve(&self, name: &str) -> Option<LocalId> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }

    fn local_ty(&self, id: LocalId) -> Type {
        self.locals[id.0].ty.clone()
    }

    fn emit(&mut self, instr: Instr) {
        self.blocks[self.active.0].instrs.push(instr);
    }

    fn terminate(&mut self, term: Term) {
        self.blocks[self.active.0].term = term;
    }

    fn is_terminated(&self) -> bool {
        !matches!(self.blocks[self.active.0].term, Term::Unreachable)
    }

    /// If the active block has no terminator, give it one and open a fresh
    /// continuation block (statements after `stop`/`give back`/`fail`).
    fn chain(&mut self, label: &str) {
        if self.is_terminated() {
            self.push_block(label);
        }
    }

    /// Create a fresh unterminated block **without** leaving the current one
    /// active — reserve the block, fill it later with `set_active`.
    fn reserve_block(&mut self, label: &str) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(Block {
            id,
            label: label.to_string(),
            instrs: Vec::new(),
            term: Term::Unreachable,
            pad_on_fail: None,
        });
        id
    }

    /// Create a fresh unterminated block and make it active (the entry block,
    /// and fall-through continuations after terminators).
    fn push_block(&mut self, label: &str) -> BlockId {
        let id = self.reserve_block(label);
        self.active = id;
        id
    }

    fn set_active(&mut self, id: BlockId) {
        self.active = id;
    }

    /// Give the active block a `Goto target` terminator if it is still open.
    /// (A block that already ended in `stop`/`give back`/`fail` needs nothing;
    /// its chained continuation block is the one that flows onward.)
    fn end_with_goto(&mut self, target: BlockId) {
        if !self.is_terminated() {
            self.terminate(Term::Goto(target));
        }
    }



    fn finish(mut self) -> MirFunction {
        // Any block still unterminated gets an implicit return. For reachable
        // straight-line ends this is the natural fall-out; sema has already
        // proved every *reachable* path gives back when one is required.
        for b in &mut self.blocks {
            if matches!(b.term, Term::Unreachable) {
                b.term = Term::Return { value: Operand::Nothing };
            }
        }
        let capabilities = if self.can_fail { vec![Capability::Fail] } else { Vec::new() };
        MirFunction {
            name: self.name,
            name_span: self.span,
            params: self.params,
            ret: self.ret,
            can_fail: self.can_fail,
            capabilities,
            locals: self.locals,
            blocks: self.blocks,
            catch_pads: self.catch_pads,
            is_entry: self.is_entry,
            span: self.span,
        }
    }

    // ----- expressions -----

    /// Evaluate `e` to an operand (an immediate when it already is one).
    fn op(&mut self, e: lagom_hir::HirExpr) -> Operand {
        match e.kind {
            lagom_hir::HirExprKind::Int(v) => Operand::Int(v),
            lagom_hir::HirExprKind::Float(v) => Operand::Float(v),
            lagom_hir::HirExprKind::Text(v) => Operand::Text(v),
            lagom_hir::HirExprKind::Bool(v) => Operand::Bool(v),
            lagom_hir::HirExprKind::Nothing => Operand::Nothing,
            lagom_hir::HirExprKind::Local(name) => match self.resolve(&name) {
                Some(id) => Operand::Local(id),
                // Unreachable in clean programs; sema rejects unknown names.
                None => Operand::Nothing,
            },
            _ => Operand::Local(self.expr(e)),
        }
    }

    /// Evaluate `e` into a fresh local and return its id.
    fn expr(&mut self, e: lagom_hir::HirExpr) -> LocalId {
        let span = e.span;
        let ty = e.ty.clone();
        match e.kind {
            lagom_hir::HirExprKind::Int(v) => self.immediate(Operand::Int(v), ty, span),
            lagom_hir::HirExprKind::Float(v) => self.immediate(Operand::Float(v), ty, span),
            lagom_hir::HirExprKind::Text(v) => self.immediate(Operand::Text(v), ty, span),
            lagom_hir::HirExprKind::Bool(v) => self.immediate(Operand::Bool(v), ty, span),
            lagom_hir::HirExprKind::Nothing => self.immediate(Operand::Nothing, ty, span),
            lagom_hir::HirExprKind::Local(name) => match self.resolve(&name) {
                Some(id) => self.immediate(Operand::Local(id), ty, span),
                None => {
                    let id = self.new_local(&name, ty.clone(), false, span);
                    self.immediate(Operand::Local(id), ty, span)
                }
            },
            lagom_hir::HirExprKind::Call { callee, callee_span, args } => {
                self.call_expr(callee, callee_span, args, ty, span)
            }
            lagom_hir::HirExprKind::Field { base, field, field_span } => {
                let dest = self.new_local("%t", ty, true, span);
                let b = self.op(*base);
                self.emit(Instr::FieldGet { dest, base: b, field, field_span, span });
                dest
            }
            lagom_hir::HirExprKind::Index { base, index } => {
                let dest = self.new_local("%t", ty, true, span);
                let b = self.op(*base);
                let i = self.op(*index);
                self.emit(Instr::IndexGet { dest, base: b, index: i, span });
                dest
            }
            lagom_hir::HirExprKind::StructLit { name, fields } => {
                let dest = self.new_local("%t", ty, true, span);
                let fields = fields
                    .into_iter()
                    .map(|(n, v, _)| {
                        let v = self.op(v);
                        (n, v)
                    })
                    .collect();
                self.emit(Instr::StructNew { dest, name, fields, span });
                dest
            }
            lagom_hir::HirExprKind::List(elems) => {
                let dest = self.new_local("%t", ty, true, span);
                let elems = elems.into_iter().map(|e| self.op(e)).collect();
                self.emit(Instr::ListNew { dest, elements: elems, span });
                dest
            }
            lagom_hir::HirExprKind::Map(entries) => {
                let dest = self.new_local("%t", ty, true, span);
                let entries = entries
                    .into_iter()
                    .map(|(k, v)| (self.op(k), self.op(v)))
                    .collect();
                self.emit(Instr::MapNew { dest, entries, span });
                dest
            }
            lagom_hir::HirExprKind::Pair(f, s) => {
                let dest = self.new_local("%t", ty, true, span);
                let f = self.op(*f);
                let s = self.op(*s);
                self.emit(Instr::PairNew { dest, first: f, second: s, span });
                dest
            }
            lagom_hir::HirExprKind::Unary { op, inner } => {
                let dest = self.new_local("%t", ty, true, span);
                let i = self.op(*inner);
                let op = match op {
                    lagom_hir::UnOp::Neg => UnOp::Neg,
                    lagom_hir::UnOp::Not => UnOp::Not,
                };
                self.emit(Instr::Unary { dest, op, operand: i, span });
                dest
            }
            lagom_hir::HirExprKind::Binary { op, left, right } => {
                let dest = self.new_local("%t", ty, true, span);
                let l = self.op(*left);
                let r = self.op(*right);
                self.emit(Instr::Binary { dest, op: lower_binop(op), left: l, right: r, span });
                dest
            }
            lagom_hir::HirExprKind::Format { parts } => {
                let dest = self.new_local("%t", ty, true, span);
                let parts = parts
                    .into_iter()
                    .map(|p| match p {
                        lagom_hir::HirFormatPart::Lit(s) => FormatPart::Lit(s),
                        lagom_hir::HirFormatPart::Value(e) => FormatPart::Value(self.op(e)),
                    })
                    .collect();
                self.emit(Instr::Format { dest, parts, span });
                dest
            }
            lagom_hir::HirExprKind::Attempt(inner) => {
                // A bare attempt in expression position (13.1/R-20.1,
                // G-15 in docs/14): the success value flows on; a failure
                // propagates out of the enclosing function (sema required
                // `can fail`). This is *not* caught here — the catch pad is
                // the statement-form tails' job. The inner call may Fail,
                // which is exactly the propagation.
                let inner_id = self.expr(*inner);
                self.immediate(Operand::Local(inner_id), ty, span)
            }
        }
    }

    fn immediate(&mut self, value: Operand, ty: Type, span: Span) -> LocalId {
        let dest = self.new_local("%t", ty, true, span);
        self.emit(Instr::Copy { dest, value, span });
        dest
    }

    fn call_expr(
        &mut self,
        callee: String,
        callee_span: Span,
        args: Vec<lagom_hir::HirExpr>,
        ty: Type,
        span: Span,
    ) -> LocalId {
        match callee.as_str() {
            "say" => {
                // `say` produces no value; a clean program never reads one.
                let a = args.into_iter().next().expect("say arity checked by sema");
                let v = self.expr(a);
                self.emit(Instr::Say { value: Operand::Local(v), span });
                self.immediate(Operand::Text(String::new()), Type::Text, span)
            }
            "ask" => {
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("ask arity checked by sema");
                let q = self.op(a);
                self.emit(Instr::Ask { dest, question: q, span });
                dest
            }
            "number" | "decimal" | "text" => {
                let conv = match callee.as_str() {
                    "number" => Conv::ToNumber,
                    "decimal" => Conv::ToDecimal,
                    _ => Conv::ToText,
                };
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("conversion arity checked");
                let v = self.op(a);
                self.emit(Instr::Convert { dest, conv, value: v, span });
                dest
            }
            "random" => {
                let dest = self.new_local("%t", ty, true, span);
                let mut it = args.into_iter();
                let lo = self.op(it.next().expect("random arity checked"));
                let hi = self.op(it.next().expect("random arity checked"));
                self.emit(Instr::Random { dest, lo, hi, span });
                dest
            }
            "first" => {
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("first arity checked");
                let v = self.op(a);
                self.emit(Instr::FirstOf { dest, list: v, span });
                dest
            }
            "size" => {
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("size arity checked");
                let v = self.op(a);
                self.emit(Instr::SizeOf { dest, value: v, span });
                dest
            }
            "join" => {
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("join arity checked");
                let v = self.op(a);
                self.emit(Instr::Join { dest, list: v, span });
                dest
            }
            "uppercase" | "lowercase" | "trim" => {
                let op = match callee.as_str() {
                    "uppercase" => TextOp::Uppercase,
                    "lowercase" => TextOp::Lowercase,
                    _ => TextOp::Trim,
                };
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("text op arity checked");
                let v = self.op(a);
                self.emit(Instr::TextOp { dest, op, value: v, span });
                dest
            }
            "square root" | "floor" => {
                let op = if callee == "square root" { MathOp::SquareRoot } else { MathOp::Floor };
                let dest = self.new_local("%t", ty, true, span);
                let a = args.into_iter().next().expect("math op arity checked");
                let v = self.op(a);
                self.emit(Instr::MathOp { dest, op, value: v, span });
                dest
            }
            _ => {
                let dest = self.new_local("%t", ty, true, span);
                let args = args.into_iter().map(|a| self.op(a)).collect();
                self.emit(Instr::Call { dest, callee, callee_span, args, span });
                dest
            }
        }
    }

    // ----- statements -----

    fn stmt(&mut self, s: lagom_hir::HirStmt) {
        let span = s.span;
        match s.kind {
            lagom_hir::HirStmtKind::Bind { mutable, name, name_span, value } => {
                let v = self.expr(value);
                let ty = self.local_ty(v);
                let id = self.new_local(&name, ty, mutable, name_span);
                self.emit(Instr::Copy { dest: id, value: Operand::Local(v), span });
                self.declare(&name, id);
            }
            lagom_hir::HirStmtKind::Assign { target, value } => {
                let v = self.expr(value);
                self.store_place(&target, Operand::Local(v), span);
            }
            lagom_hir::HirStmtKind::AssignOp { target, increase, value } => {
                // Read-modify-write: the one mutation form. `increase`/
                // `decrease` stay distinct at HIR for LOM/overflow checks;
                // MIR expands to load/op/store.
                let rhs = self.expr(value);
                let op = if increase { BinOp::Add } else { BinOp::Sub };
                self.read_modify_write(&target, rhs, op, span);
            }
            lagom_hir::HirStmtKind::If { branches, otherwise } => {
                self.lower_if(branches, otherwise)
            }
            lagom_hir::HirStmtKind::RepeatCount { times, times_span: _, binding, body } => {
                self.lower_count_loop(times, binding, body, span)
            }
            lagom_hir::HirStmtKind::RepeatWhile { cond, body } => {
                self.lower_while_loop(cond, body)
            }
            lagom_hir::HirStmtKind::RepeatForEach { item, index, iter, body } => {
                self.lower_for_each(item, index, iter, body, span)
            }
            lagom_hir::HirStmtKind::Stop => {
                let (join, _) = *self
                    .loop_targets
                    .last()
                    .expect("`stop` outside a loop is rejected by sema");
                self.terminate(Term::Break { target: join });
                self.chain("after stop");
            }
            lagom_hir::HirStmtKind::Next => {
                let (_, cond) = *self
                    .loop_targets
                    .last()
                    .expect("`next` outside a loop is rejected by sema");
                self.terminate(Term::Continue { target: cond });
                self.chain("after next");
            }
            lagom_hir::HirStmtKind::Return { value } => {
                let v = self.expr(value);
                self.terminate(Term::Return { value: Operand::Local(v) });
                self.chain("after give back");
            }
            lagom_hir::HirStmtKind::Fail { message } => {
                let m = self.expr(message);
                self.terminate(Term::Fail {
                    message: Operand::Local(m),
                    catch: self.catches.last().copied(),
                    span,
                });
                self.chain("after fail");
            }
            lagom_hir::HirStmtKind::Attempt { inner, tail } => {
                self.lower_attempt(inner, tail, span)
            }
            lagom_hir::HirStmtKind::Check { expr, cmp } => {
                // G-4: when the checked expression is a comparison, evaluate
                // its two sides once and reuse the operands for both the
                // boolean value and the failure report.
                let (value, cmp_mir) = match cmp {
                    Some((op, l, r)) => {
                        let op = lower_binop(op);
                        let lv = self.expr(*l);
                        let rv = self.expr(*r);
                        let dest = self.new_local("%cond", Type::Boolean, true, span);
                        self.emit(Instr::Binary {
                            dest,
                            op,
                            left: Operand::Local(lv),
                            right: Operand::Local(rv),
                            span,
                        });
                        (
                            dest,
                            Some((op, Operand::Local(lv), Operand::Local(rv))),
                        )
                    }
                    None => (self.expr(expr), None),
                };
                self.emit(Instr::Check { value: Operand::Local(value), cmp: cmp_mir, span });
            }
            lagom_hir::HirStmtKind::Effect(e) => self.effect(e, span),
        }
    }

    fn effect(&mut self, e: lagom_hir::HirExpr, span: Span) {
        match e.kind {
            lagom_hir::HirExprKind::Call { callee, args, .. } if callee == "say" => {
                let a = args.into_iter().next().expect("say arity checked by sema");
                let v = self.expr(a);
                self.emit(Instr::Say { value: Operand::Local(v), span });
            }
            lagom_hir::HirExprKind::Call { callee, callee_span, args } => {
                // A user call for effect (value unused). Builtins have no
                // side effects at M0.
                let dest = self.new_local("%ignored", Type::Error, true, span);
                let args = args.into_iter().map(|a| self.op(a)).collect();
                self.emit(Instr::Call { dest, callee, callee_span, args, span });
            }
            _ => {
                self.expr(e);
            }
        }
    }

    // ----- control flow -----

    /// `if`-chains: the condition evaluates in the block that precedes the
    /// branch (straight-line flow), each body falls into the shared join.
    fn lower_if(
        &mut self,
        branches: Vec<(lagom_hir::HirExpr, Vec<lagom_hir::HirStmt>, Span)>,
        otherwise: Option<Vec<lagom_hir::HirStmt>>,
    ) {
        let join = self.reserve_block("if join");
        for (i, (cond, body, _bspan)) in branches.into_iter().enumerate() {
            // Active block: the pre-if block (first branch) or the previous
            // branch's fallthrough test block. Evaluate the condition here,
            // then branch.
            let c = self.expr(cond);
            let then_b = self.reserve_block(&format!("if body {i}"));
            let next_test = self.reserve_block(&format!("if test {}", i + 1));
            self.terminate(Term::Branch {
                cond: Operand::Local(c),
                then: then_b,
                otherwise: next_test,
            });
            // Fill the body.
            self.set_active(then_b);
            for s in body {
                self.stmt(s);
            }
            self.end_with_goto(join);
            // Continue the test chain.
            self.set_active(next_test);
        }
        // Otherwise path: the last test block falls through here.
        if let Some(else_body) = otherwise {
            for s in else_body {
                self.stmt(s);
            }
        }
        self.end_with_goto(join);
        self.set_active(join);
    }

    fn lower_count_loop(
        &mut self,
        times: i64,
        binding: Option<(String, Span)>,
        body: Vec<lagom_hir::HirStmt>,
        span: Span,
    ) {
        // init (active) → test → body → increment → test … ; join after.
        let times_local = self.new_local("%times", Type::Number, true, span);
        self.emit(Instr::Copy { dest: times_local, value: Operand::Int(times), span });
        let counter = self.new_local("%i", Type::Number, true, span);
        self.emit(Instr::Copy { dest: counter, value: Operand::Int(0), span });

        let test = self.reserve_block("loop test");
        let incr = self.reserve_block("loop increment");
        let join = self.reserve_block("loop join");
        self.terminate(Term::Goto(test));

        self.set_active(test);
        let cond = self.new_local("%cond", Type::Boolean, true, span);
        let i_copy = self.new_local("%i_copy", Type::Number, true, span);
        self.emit(Instr::Copy { dest: i_copy, value: Operand::Local(counter), span });
        self.emit(Instr::Binary {
            dest: cond,
            op: BinOp::Less,
            left: Operand::Local(i_copy),
            right: Operand::Local(times_local),
            span,
        });
        let body_b = self.reserve_block("loop body");
        self.terminate(Term::Branch { cond: Operand::Local(cond), then: body_b, otherwise: join });

        self.set_active(body_b);
        if let Some((name, name_span)) = binding {
            let id = self.new_local(&name, Type::Number, false, name_span);
            self.emit(Instr::Copy { dest: id, value: Operand::Local(counter), span });
            self.declare(&name, id);
        }
        self.scopes.push(std::collections::HashMap::new());
        self.loop_targets.push((join, incr));
        for s in body {
            self.stmt(s);
        }
        self.loop_targets.pop();
        self.scopes.pop();
        self.end_with_goto(incr);

        // Increment: counter = counter + 1, then back to the test.
        self.set_active(incr);
        let one = self.new_local("%one", Type::Number, true, span);
        self.emit(Instr::Copy { dest: one, value: Operand::Int(1), span });
        let next_i = self.new_local("%next_i", Type::Number, true, span);
        self.emit(Instr::Binary {
            dest: next_i,
            op: BinOp::Add,
            left: Operand::Local(counter),
            right: Operand::Local(one),
            span,
        });
        self.emit(Instr::Copy { dest: counter, value: Operand::Local(next_i), span });
        self.terminate(Term::Goto(test));

        self.set_active(join);
    }

    fn lower_while_loop(&mut self, cond: lagom_hir::HirExpr, body: Vec<lagom_hir::HirStmt>) {
        let test = self.reserve_block("while test");
        let join = self.reserve_block("while join");
        self.terminate(Term::Goto(test));

        self.set_active(test);
        let c = self.expr(cond);
        let body_b = self.reserve_block("while body");
        self.terminate(Term::Branch { cond: Operand::Local(c), then: body_b, otherwise: join });

        self.set_active(body_b);
        self.scopes.push(std::collections::HashMap::new());
        self.loop_targets.push((join, test));
        for s in body {
            self.stmt(s);
        }
        self.loop_targets.pop();
        self.scopes.pop();
        self.end_with_goto(test);

        self.set_active(join);
    }

    fn lower_for_each(
        &mut self,
        item: (String, Span),
        index: Option<(String, Span)>,
        iter: lagom_hir::HirExpr,
        body: Vec<lagom_hir::HirStmt>,
        span: Span,
    ) {
        // M0 desugars `repeat for each x in list` to index iteration over
        // `size of list` — one loop form reaches the backend.
        let list = self.expr(iter);
        let list_ty = self.local_ty(list);
        let size_local = self.new_local("%size", Type::Number, true, span);
        self.emit(Instr::SizeOf { dest: size_local, value: Operand::Local(list), span });
        let counter = self.new_local("%i", Type::Number, true, span);
        self.emit(Instr::Copy { dest: counter, value: Operand::Int(0), span });

        let test = self.reserve_block("for each test");
        let incr = self.reserve_block("for each increment");
        let join = self.reserve_block("for each join");
        self.terminate(Term::Goto(test));

        self.set_active(test);
        let cond = self.new_local("%cond", Type::Boolean, true, span);
        let i_copy = self.new_local("%i_copy", Type::Number, true, span);
        self.emit(Instr::Copy { dest: i_copy, value: Operand::Local(counter), span });
        self.emit(Instr::Binary {
            dest: cond,
            op: BinOp::Less,
            left: Operand::Local(i_copy),
            right: Operand::Local(size_local),
            span,
        });
        let body_b = self.reserve_block("for each body");
        self.terminate(Term::Branch { cond: Operand::Local(cond), then: body_b, otherwise: join });

        self.set_active(body_b);
        let elem = self.new_local("%elem", element_of(&list_ty), true, span);
        self.emit(Instr::IndexGet {
            dest: elem,
            base: Operand::Local(list),
            index: Operand::Local(counter),
            span,
        });
        let (item_name, item_span) = item;
        let item_local = self.new_local(&item_name, element_of(&list_ty), false, item_span);
        self.emit(Instr::Copy { dest: item_local, value: Operand::Local(elem), span });
        self.declare(&item_name, item_local);
        if let Some((iname, ispan)) = index {
            let il = self.new_local(&iname, Type::Number, false, ispan);
            self.emit(Instr::Copy { dest: il, value: Operand::Local(counter), span });
            self.declare(&iname, il);
        }
        self.scopes.push(std::collections::HashMap::new());
        self.loop_targets.push((join, incr));
        for s in body {
            self.stmt(s);
        }
        self.loop_targets.pop();
        self.scopes.pop();
        self.end_with_goto(incr);

        // Increment: counter = counter + 1, then back to the test.
        self.set_active(incr);
        let one = self.new_local("%one", Type::Number, true, span);
        self.emit(Instr::Copy { dest: one, value: Operand::Int(1), span });
        let next_i = self.new_local("%next_i", Type::Number, true, span);
        self.emit(Instr::Binary {
            dest: next_i,
            op: BinOp::Add,
            left: Operand::Local(counter),
            right: Operand::Local(one),
            span,
        });
        self.emit(Instr::Copy { dest: counter, value: Operand::Local(next_i), span });
        self.terminate(Term::Goto(test));

        self.set_active(join);
    }

    // ----- attempts (13.1) -----

    fn lower_attempt(
        &mut self,
        inner: lagom_hir::HirExpr,
        tail: Option<lagom_hir::HirAttemptTail>,
        span: Span,
    ) {
        match tail {
            None => self.attempt_bare(inner, span),
            Some(lagom_hir::HirAttemptTail::Propagate) => self.attempt_propagate(inner, span),
            Some(lagom_hir::HirAttemptTail::IfItFails { then_block, otherwise }) => {
                self.attempt_if_it_fails(inner, then_block, otherwise, span)
            }
            Some(lagom_hir::HirAttemptTail::As { name, name_span, block, otherwise }) => {
                self.attempt_as(inner, name, name_span, block, otherwise, span)
            }
        }
    }

    /// Shared try/pad setup: reserves the join and catch blocks, evaluates
    /// `inner` in the **current** (pre-attempt) block — it may `Fail` to the
    /// pad, exactly the propagation 13.1 describes — then terminates the
    /// pre-block with `Goto(cont)` for the success path. Leaves the pad
    /// active with `problem` registered in `catch_pads` (not in scopes — the
    /// tail decides that). Returns `(join, pad, result, problem)`.
    fn attempt_try(
        &mut self,
        inner: lagom_hir::HirExpr,
        span: Span,
    ) -> (BlockId, BlockId, LocalId, LocalId) {
        let pad = self.reserve_block("attempt catch");
        self.catches.push(pad);
        // The inner expression evaluates in its OWN block with `pad_on_fail`
        // set: instruction-level failures (a failing `Call`/`Convert`) land
        // exactly on this attempt's pad — not on any attempt surrounding the
        // whole statement.
        let try_b = self.reserve_block("attempt try");
        self.blocks[try_b.0].pad_on_fail = Some(pad);
        // The pre-attempt block flows into the try block — the attempt's
        // inner evaluation is its own block so instruction-level failures
        // route exactly to this pad (and `finish` cannot mistake the
        // pre-attempt block for a fallen-off end).
        self.terminate(Term::Goto(try_b));
        self.set_active(try_b);
        let result = self.expr(inner);
        // Success continues in its own block — distinct from the failure
        // branch, so the pad's body can fall through without running the
        // success body.
        let succ = self.reserve_block("attempt success");
        self.terminate(Term::Goto(succ));
        // The pad: the runtime writes the caught message into `problem`.
        self.set_active(pad);
        let problem = self.new_local("problem", Type::Text, false, span);
        self.catch_pads.push((pad, problem));
        self.catches.pop();
        (succ, pad, result, problem)
    }

    /// `attempt <expr>` — the bare statement form: failures inside the
    /// attempt are handled here (the pad catches them), but the pad itself
    /// re-fails outward — a bare attempt handles the boundary, not the
    /// problem (13.1: a bare attempt propagates; sema required `can fail`).
    fn attempt_bare(&mut self, inner: lagom_hir::HirExpr, span: Span) {
        let (cont, _pad, _result, problem) = self.attempt_try(inner, span);
        // The pad is still active (attempt_try leaves it so). A caught
        // failure re-fails outward toward the *outer* pad, or leaves the
        // function (`catch: None`) when there is none — 13.1's propagation.
        self.terminate(Term::Fail {
            message: Operand::Local(problem),
            catch: self.catches.last().copied(),
            span,
        });
        self.set_active(cont);
    }

    /// `attempt <expr> and pass the problem on` — re-fail from the pad toward
    /// the *outer* pad (or out of the function; sema required `can fail`).
    fn attempt_propagate(&mut self, inner: lagom_hir::HirExpr, span: Span) {
        let (cont, pad, _result, problem) = self.attempt_try(inner, span);
        self.set_active(pad);
        // `catches` was popped inside `attempt_try`, so `.last()` is the
        // *outer* pad; none means the failure leaves this function.
        self.terminate(Term::Fail {
            message: Operand::Local(problem),
            catch: self.catches.last().copied(),
            span,
        });
        self.set_active(cont);
    }

    /// `attempt <expr> if it fails then … otherwise …` — `problem` bound in
    /// the failure branch, `result` in the success branch (13.1).
    fn attempt_if_it_fails(
        &mut self,
        inner: lagom_hir::HirExpr,
        then_block: Vec<lagom_hir::HirStmt>,
        otherwise: Option<Vec<lagom_hir::HirStmt>>,
        span: Span,
    ) {
        let (succ, pad, result, problem) = self.attempt_try(inner, span);
        let join = self.reserve_block("attempt join");
        // Failure branch (from the pad).
        self.set_active(pad);
        self.scopes.push(std::collections::HashMap::new());
        self.declare("problem", problem);
        for s in then_block {
            self.stmt(s);
        }
        self.scopes.pop();
        self.end_with_goto(join);
        // Success branch (its own block, where `result` is bound).
        self.set_active(succ);
        self.scopes.push(std::collections::HashMap::new());
        self.declare("result", result);
        for s in otherwise.into_iter().flatten() {
            self.stmt(s);
        }
        self.scopes.pop();
        self.end_with_goto(join);
        self.set_active(join);
    }

    /// `attempt <expr> as <name> … otherwise …` — the error bound under the
    /// chosen name (13.1).
    fn attempt_as(
        &mut self,
        inner: lagom_hir::HirExpr,
        name: String,
        name_span: Span,
        block: Vec<lagom_hir::HirStmt>,
        otherwise: Option<Vec<lagom_hir::HirStmt>>,
        span: Span,
    ) {
        let (succ, pad, result, problem) = self.attempt_try(inner, span);
        let join = self.reserve_block("attempt join");
        self.set_active(pad);
        let as_local = self.new_local(&name, Type::Text, false, name_span);
        self.emit(Instr::Copy { dest: as_local, value: Operand::Local(problem), span });
        self.scopes.push(std::collections::HashMap::new());
        self.declare(&name, as_local);
        for s in block {
            self.stmt(s);
        }
        self.scopes.pop();
        self.end_with_goto(join);
        self.set_active(succ);
        self.scopes.push(std::collections::HashMap::new());
        self.declare("result", result);
        for s in otherwise.into_iter().flatten() {
            self.stmt(s);
        }
        self.scopes.pop();
        self.end_with_goto(join);
        self.set_active(join);
    }

    // ----- places -----

    fn store_place(&mut self, target: &lagom_hir::HirPlace, value: Operand, span: Span) {
        let base = self
            .resolve(&target.base)
            .unwrap_or_else(|| self.new_local(&target.base, Type::Error, true, target.base_span));
        if target.path.is_empty() {
            // A plain variable: the place *is* the binding's local.
            self.emit(Instr::Copy { dest: base, value, span });
            return;
        }
        let mut cur_base = Operand::Local(base);
        let n = target.path.len();
        for (i, access) in target.path.iter().enumerate() {
            let last = i + 1 == n;
            match access {
                lagom_hir::HirAccess::Field(field, fspan) => {
                    if last {
                        self.emit(Instr::FieldSet {
                            base: cur_base.clone(),
                            field: field.clone(),
                            value: value.clone(),
                            span,
                        });
                    } else {
                        let tmp = self.new_local("%fp", Type::Error, true, span);
                        self.emit(Instr::FieldGet {
                            dest: tmp,
                            base: cur_base.clone(),
                            field: field.clone(),
                            field_span: *fspan,
                            span,
                        });
                        cur_base = Operand::Local(tmp);
                    }
                }
                lagom_hir::HirAccess::Index(idx) => {
                    let idx_op = self.op(idx.clone());
                    if last {
                        self.emit(Instr::IndexSet {
                            base: cur_base.clone(),
                            index: idx_op,
                            value: value.clone(),
                            span,
                        });
                    } else {
                        let tmp = self.new_local("%ip", Type::Error, true, span);
                        self.emit(Instr::IndexGet {
                            dest: tmp,
                            base: cur_base.clone(),
                            index: idx_op,
                            span,
                        });
                        cur_base = Operand::Local(tmp);
                    }
                }
            }
        }
    }

    fn read_modify_write(
        &mut self,
        target: &lagom_hir::HirPlace,
        rhs: LocalId,
        op: BinOp,
        span: Span,
    ) {
        let base = self
            .resolve(&target.base)
            .unwrap_or_else(|| self.new_local(&target.base, Type::Error, true, target.base_span));
        let n = target.path.len();
        if n == 0 {
            let sum = self.new_local("%sum", Type::Error, true, span);
            self.emit(Instr::Binary {
                dest: sum,
                op,
                left: Operand::Local(base),
                right: Operand::Local(rhs),
                span,
            });
            self.emit(Instr::Copy { dest: base, value: Operand::Local(sum), span });
            return;
        }
        let mut cur_base = Operand::Local(base);
        for (i, access) in target.path.iter().enumerate() {
            let last = i + 1 == n;
            match access {
                lagom_hir::HirAccess::Field(field, fspan) => {
                    let tmp = self.new_local("%rmw", Type::Error, true, span);
                    self.emit(Instr::FieldGet {
                        dest: tmp,
                        base: cur_base.clone(),
                        field: field.clone(),
                        field_span: *fspan,
                        span,
                    });
                    if last {
                        let sum = self.new_local("%sum", Type::Error, true, span);
                        self.emit(Instr::Binary {
                            dest: sum,
                            op,
                            left: Operand::Local(tmp),
                            right: Operand::Local(rhs),
                            span,
                        });
                        self.emit(Instr::FieldSet {
                            base: cur_base.clone(),
                            field: field.clone(),
                            value: Operand::Local(sum),
                            span,
                        });
                    } else {
                        cur_base = Operand::Local(tmp);
                    }
                }
                lagom_hir::HirAccess::Index(idx) => {
                    let idx_op = self.op(idx.clone());
                    let tmp = self.new_local("%rmw", Type::Error, true, span);
                    self.emit(Instr::IndexGet {
                        dest: tmp,
                        base: cur_base.clone(),
                        index: idx_op.clone(),
                        span,
                    });
                    if last {
                        let sum = self.new_local("%sum", Type::Error, true, span);
                        self.emit(Instr::Binary {
                            dest: sum,
                            op,
                            left: Operand::Local(tmp),
                            right: Operand::Local(rhs),
                            span,
                        });
                        self.emit(Instr::IndexSet {
                            base: cur_base.clone(),
                            index: idx_op,
                            value: Operand::Local(sum),
                            span,
                        });
                    } else {
                        cur_base = Operand::Local(tmp);
                    }
                }
            }
        }
    }
}

fn element_of(ty: &Type) -> Type {
    match ty {
        Type::List(e) => (**e).clone(),
        _ => Type::Error,
    }
}

fn lower_binop(op: lagom_hir::BinOp) -> BinOp {
    use lagom_hir::BinOp as H;
    match op {
        H::Add => BinOp::Add,
        H::Sub => BinOp::Sub,
        H::Mul => BinOp::Mul,
        H::Div => BinOp::Div,
        H::DivEvenly => BinOp::DivEvenly,
        H::Rem => BinOp::Rem,
        H::And => BinOp::And,
        H::Or => BinOp::Or,
        H::Equal => BinOp::Equal,
        H::NotEqual => BinOp::NotEqual,
        H::Greater => BinOp::Greater,
        H::Less => BinOp::Less,
        H::AtLeast => BinOp::AtLeast,
        H::AtMost => BinOp::AtMost,
    }
}

// ---------------------------------------------------------------------------
// The LOM instrumentation pass (§26.5)
// ---------------------------------------------------------------------------

/// Add the LOM probes a build mode needs. This is *the* named pass doc 08
/// owns: dev builds get function-entry snapshots, binding provenance, and
/// failure events; release builds are stripped of any probes and stay silent
/// (the release-identity invariant — a tested property, §26.5).
///
/// Returns the number of probes present for dev (0 for release).
pub fn instrument(prog: &mut MirProgram, cfg: LomConfig) -> usize {
    match cfg {
        LomConfig { dev: false } => {
            // Release: strip any probe that could have arrived (idempotent)
            // and emit nothing — "release builds are silent".
            for item in &mut prog.items {
                let f = match item {
                    MirItem::Main(f) | MirItem::Function(f) | MirItem::Test(f) => f,
                };
                for b in &mut f.blocks {
                    b.instrs.retain(|i| !is_event(i));
                }
            }
            0
        }
        LomConfig { dev: true } => {
            let mut added = 0;
            for item in &mut prog.items {
                let f = match item {
                    MirItem::Main(f) | MirItem::Function(f) | MirItem::Test(f) => f,
                };
                // Function entry: snapshot the parameters (entries have none).
                if !f.params.is_empty() {
                    let args: Vec<(String, Operand)> = f
                        .params
                        .iter()
                        .map(|id| (f.local(*id).name.clone(), Operand::Local(*id)))
                        .collect();
                    if let Some(entry) = f.blocks.first_mut() {
                        entry
                            .instrs
                            .insert(0, Instr::EventFunctionEntry { function: f.name.clone(), args });
                        added += 1;
                    }
                }
                // Binding provenance: after every Copy into a source binding
                // (not a `%temp`), record where the value came from.
                let mut bind_probes: Vec<(usize, usize, Instr)> = Vec::new();
                for (bi, b) in f.blocks.iter().enumerate() {
                    for (ii, instr) in b.instrs.iter().enumerate() {
                        if let Instr::Copy { dest, value, .. } = instr {
                            let local = f.local(*dest);
                            if local.is_source_binding() {
                                bind_probes.push((
                                    bi,
                                    ii + 1,
                                    Instr::EventBind {
                                        name: local.name.clone(),
                                        value: value.clone(),
                                        site: local.span,
                                    },
                                ));
                            }
                        }
                    }
                }
                for (bi, ii, probe) in bind_probes.into_iter().rev() {
                    f.blocks[bi].instrs.insert(ii, probe);
                    added += 1;
                }
                // Failure events: before every Fail terminator.
                for b in &mut f.blocks {
                    if let Term::Fail { message, .. } = &b.term {
                        b.instrs.push(Instr::EventFail { message: message.clone() });
                        added += 1;
                    }
                }
            }
            added
        }
    }
}

pub fn is_event(i: &Instr) -> bool {
    matches!(
        i,
        Instr::EventFunctionEntry { .. } | Instr::EventBind { .. } | Instr::EventFail { .. }
    )
}

// ---------------------------------------------------------------------------
// The event ring and the failure report (§26.5, doc 09's v1 shape)
// ---------------------------------------------------------------------------

/// The bounded in-memory event ring (fixed budget, oldest evicted — 26.5's
/// "no unbounded recording, ever").
pub struct EventRing {
    events: Vec<LomEvent>,
    capacity: usize,
}

impl EventRing {
    pub fn new(capacity: usize) -> EventRing {
        EventRing { events: Vec::new(), capacity }
    }

    pub fn push(&mut self, event: LomEvent) {
        if self.capacity == 0 {
            return;
        }
        if self.events.len() >= self.capacity {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    pub fn events(&self) -> &[LomEvent] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// One recorded event. Values are pre-rendered by the runtime (it owns S-9's
/// formatting); `site` is the byte offset of the binding's source span.
#[derive(Debug, Clone, PartialEq)]
pub enum LomEvent {
    FunctionEntry { function: String, args: Vec<(String, String)> },
    Bind { name: String, value: String, site: usize },
    Failure { message: String },
}

/// Render a dev-build failure report from the ring (26.5's payoff artifact;
/// doc 09 owns the final format — M0 ships the v1 shape). `src` turns offsets
/// into line numbers ("bound at line 12").
pub fn failure_report(ring: &EventRing, function: &str, src: &str) -> String {
    let mut out = String::new();
    out.push_str("— what happened —\n");
    out.push_str(&format!("in `{function}`, a failure was not handled.\n"));
    if ring.is_empty() {
        out.push_str("(no recorded events — the failure happened before any binding was recorded.)\n");
        return out;
    }
    out.push_str("— what the program was doing —\n");
    for e in ring.events() {
        match e {
            LomEvent::FunctionEntry { function, args } => {
                out.push_str(&format!("called `{function}` with {}.\n", fmt_args(args)));
            }
            LomEvent::Bind { name, value, site } => {
                let line = line_of(src, *site);
                out.push_str(&format!("`{name}` became {value} (line {line}).\n"));
            }
            LomEvent::Failure { message } => {
                out.push_str(&format!("it failed with: {message}\n"));
            }
        }
    }
    out
}

fn fmt_args(args: &[(String, String)]) -> String {
    if args.is_empty() {
        return "no arguments".to_string();
    }
    args.iter()
        .map(|(n, v)| format!("`{n}` = {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The 1-based line containing byte offset `off`.
pub fn line_of(src: &str, off: usize) -> usize {
    src[..off.min(src.len())].bytes().filter(|b| *b == b'\n').count() + 1
}

// ---------------------------------------------------------------------------
// Verifier (§22.4)
// ---------------------------------------------------------------------------

/// Check the IR invariants: no unterminated blocks, dense ordered ids, only
/// in-range edges. `lagom build --verify-ir` runs this (22.4).
pub fn verify(prog: &MirProgram) -> Result<(), String> {
    for item in &prog.items {
        let f = match item {
            MirItem::Main(f) | MirItem::Function(f) | MirItem::Test(f) => f,
        };
        if f.blocks.is_empty() {
            return Err(format!("`{}` has no blocks", f.name));
        }
        for (i, b) in f.blocks.iter().enumerate() {
            if b.id.0 != i {
                return Err(format!(
                    "`{}` block {} is out of order (expected {i})",
                    f.name, b.id.0
                ));
            }
            if matches!(b.term, Term::Unreachable) {
                return Err(format!(
                    "`{}` block {} (\"{}\") has no terminator",
                    f.name, b.id.0, b.label
                ));
            }
            match &b.term {
                Term::Branch { then, otherwise, .. } => {
                    if then.0 >= f.blocks.len() || otherwise.0 >= f.blocks.len() {
                        return Err(format!("`{}` block {} branches out of range", f.name, b.id.0));
                    }
                }
                Term::Goto(t) | Term::Break { target: t, .. } | Term::Continue { target: t, .. } => {
                    if t.0 >= f.blocks.len() {
                        return Err(format!("`{}` block {} jumps out of range", f.name, b.id.0));
                    }
                }
                _ => {}
            }
        }
        for pad in &f.catch_pads {
            if pad.0 .0 >= f.blocks.len() {
                return Err(format!("`{}` has a catch pad out of range", f.name));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — the canonical corpus, lowered and verified
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lagom_hir as hir;
    use lagom_parser::parse;
    use lagom_sema::check;

    fn build(src: &str) -> MirProgram {
        let (ast, pdiags) = parse(src);
        assert!(pdiags.is_empty(), "parse errors in test source: {pdiags:?}");
        let checked = check(&ast, src);
        assert!(
            checked.diags.items.is_empty(),
            "sema errors in test source: {:?}",
            checked
                .diags
                .items
                .iter()
                .map(|d| (&d.code, &d.message))
                .collect::<Vec<_>>()
        );
        let h = hir::lower(checked);
        let m = lower(h);
        verify(&m).expect("MIR must verify");
        m
    }

    fn main_of(prog: &MirProgram) -> &MirFunction {
        prog.items
            .iter()
            .find_map(|i| match i {
                MirItem::Main(f) => Some(f),
                _ => None,
            })
            .expect("the program has a script body")
    }

    fn instrs(f: &MirFunction) -> Vec<&Instr> {
        f.blocks.iter().flat_map(|b| b.instrs.iter()).collect()
    }

    // ----- hello world, end to end -----

    #[test]
    fn hello_world_end_to_end() {
        let m = build("say \"hello world\"");
        let f = main_of(&m);
        let says = instrs(f).iter().filter(|i| matches!(i, Instr::Say { .. })).count();
        assert_eq!(says, 1);
    }

    #[test]
    fn hello_world_through_every_stage() {
        // AST → sema → HIR → MIR on one canonical line.
        let src = "say \"hello world\"";
        let (ast, _) = parse(src);
        let h = hir::lower(check(&ast, src));
        assert!(matches!(h.items[0], hir::HirItem::Main(_)));
        let m = lower(h);
        verify(&m).unwrap();
        assert_eq!(m.items.len(), 1);
    }

    // ----- bindings and arithmetic -----

    #[test]
    fn bind_lowers_to_copy_with_named_local() {
        let m = build("make score equal to 5\nsay score");
        let f = main_of(&m);
        assert!(f.locals.iter().any(|l| l.name == "score" && !l.mutable));
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::Say { .. })));
    }

    #[test]
    fn mutable_binding_is_marked() {
        let m = build("make changing lives equal to 3");
        let f = main_of(&m);
        let lives = f.locals.iter().find(|l| l.name == "lives").expect("lives local");
        assert!(lives.mutable);
    }

    #[test]
    fn arithmetic_lowers_to_binary_chain() {
        let m = build("say 2 plus 3 times 4");
        let f = main_of(&m);
        let bins: Vec<_> = instrs(f)
            .iter()
            .filter_map(|i| match i {
                Instr::Binary { op, .. } => Some(*op),
                _ => None,
            })
            .collect();
        assert_eq!(bins, vec![BinOp::Mul, BinOp::Add], "times binds first (7.3)");
    }

    #[test]
    fn interpolation_becomes_format_parts() {
        let m = build(
            "make name equal to \"bo\"\nsay \"hi {name}, you have {2 plus 3} points\"",
        );
        let f = main_of(&m);
        let fmt = instrs(f)
            .iter()
            .find_map(|i| match i {
                Instr::Format { parts, .. } => Some(parts),
                _ => None,
            })
            .expect("a Format instruction");
        assert_eq!(fmt.len(), 5, "Lit, Value, Lit, Value, Lit");
        assert!(matches!(&fmt[0], FormatPart::Lit(s) if s.contains("hi ")));
        assert!(matches!(&fmt[1], FormatPart::Value(_)));
        assert!(matches!(&fmt[3], FormatPart::Value(_)));
    }

    // ----- control flow -----

    #[test]
    fn count_loop_has_test_body_join_blocks() {
        let m = build("repeat 3 times using i\n    say i");
        let f = main_of(&m);
        let labels: Vec<_> = f.blocks.iter().map(|b| b.label.clone()).collect();
        assert!(labels.iter().any(|l| l == "loop test"), "labels: {labels:?}");
        assert!(labels.iter().any(|l| l == "loop body"), "labels: {labels:?}");
        assert!(labels.iter().any(|l| l == "loop join"), "labels: {labels:?}");
        let test = f.blocks.iter().find(|b| b.label == "loop test").unwrap();
        assert!(matches!(test.term, Term::Branch { .. }));
        let incr = f.blocks.iter().find(|b| b.label == "loop increment").unwrap();
        assert!(matches!(incr.term, Term::Goto(t) if t == test.id));
        assert!(f.locals.iter().any(|l| l.name == "i" && !l.mutable));
    }

    #[test]
    fn while_loop_returns_to_test() {
        let m = build(
            "make changing n equal to 3\nrepeat while n is greater than 0\n    set n to n minus 1",
        );
        let f = main_of(&m);
        let test = f.blocks.iter().find(|b| b.label == "while test").unwrap();
        // The body falls through back to the test (its own block when the
        // body has no terminators).
        let body = f.blocks.iter().find(|b| b.label == "while body").unwrap();
        assert!(matches!(body.term, Term::Goto(t) if t == test.id), "body term: {:?}", body.term);
    }

    #[test]
    fn for_each_desugars_to_index_loop() {
        let m = build(
            "make things equal to a list of 1, 2, 3\nrepeat for each t in things\n    say t",
        );
        let f = main_of(&m);
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::SizeOf { .. })));
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::IndexGet { .. })));
        assert!(f.locals.iter().any(|l| l.name == "t"));
    }

    #[test]
    fn stop_breaks_to_join_and_next_continues_to_test() {
        let m = build("repeat 10 times\n    if 2 is greater than 1\n        stop");
        let f = main_of(&m);
        let join = f.blocks.iter().find(|b| b.label == "loop join").unwrap();
        let brk = f
            .blocks
            .iter()
            .find(|b| matches!(b.term, Term::Break { .. }))
            .expect("a Break terminator");
        match &brk.term {
            Term::Break { target } => assert_eq!(*target, join.id),
            _ => unreachable!(),
        }

        let m2 = build(
            "make changing i equal to 0\nrepeat while i is less than 10\n    set i to i plus 1\n    next",
        );
        let f2 = main_of(&m2);
        let test = f2.blocks.iter().find(|b| b.label == "while test").unwrap();
        let cont = f2
            .blocks
            .iter()
            .find(|b| matches!(b.term, Term::Continue { .. }))
            .expect("a Continue terminator");
        match &cont.term {
            Term::Continue { target } => assert_eq!(*target, test.id),
            _ => unreachable!(),
        }
    }

    // ----- data -----

    #[test]
    fn list_and_index_read() {
        let m = build("make things equal to a list of 1, 2, 3\nsay things at 2");
        let f = main_of(&m);
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::ListNew { .. })));
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::IndexGet { .. })));
    }

    #[test]
    fn struct_new_and_field_read() {
        let m = build(
            "structure player\n    has name of type text\n    has score of type number\nmake p equal to a player with name \"bo\" and score 0\nsay name of p",
        );
        let f = main_of(&m);
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::StructNew { .. })));
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::FieldGet { .. })));
        let s = m.structs.iter().find(|s| s.name == "player").expect("player struct");
        assert_eq!(
            s.fields,
            vec![("name".to_string(), Type::Text), ("score".to_string(), Type::Number)]
        );
    }

    #[test]
    fn field_and_index_writes() {
        let m = build(
            "structure player\n    has name of type text\n    has score of type number\nmake changing p equal to a player with name \"bo\" and score 0\nset score of p to 10\nmake changing things equal to a list of \"a\", \"b\"\nset things at 1 to \"c\"",
        );
        let f = main_of(&m);
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::FieldSet { .. })));
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::IndexSet { .. })));
    }

    #[test]
    fn increase_is_read_modify_write() {
        let m = build("make changing score equal to 1\nincrease score by 5");
        let f = main_of(&m);
        let adds: Vec<_> = instrs(f)
            .iter()
            .filter_map(|i| match i {
                Instr::Binary { op, .. } => Some(*op),
                _ => None,
            })
            .collect();
        assert!(adds.contains(&BinOp::Add), "expected an Add in {adds:?}");
    }

    // ----- errors and conversions -----

    #[test]
    fn conversions_become_convert_and_ask() {
        let m = build(
            "make answer equal to ask \"pick\"\nattempt number from answer if it fails then\n    say \"no\"\notherwise\n    say result",
        );
        let f = main_of(&m);
        assert!(instrs(f).iter().any(|i| matches!(i, Instr::Ask { .. })));
        assert!(
            instrs(f).iter().any(|i| matches!(i, Instr::Convert { conv: Conv::ToNumber, .. })),
            "the attempt's inner conversion lowers to Convert"
        );
    }

    #[test]
    fn attempt_with_tail_builds_catch_pad() {
        let m = build(
            "function divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    give back top divided by bottom\nattempt divide 10 and 0 if it fails then\n    say \"no\"\notherwise\n    say \"ok\"",
        );
        let f = main_of(&m);
        assert_eq!(f.catch_pads.len(), 1);
        let (pad, problem) = f.catch_pads[0];
        assert_eq!(f.local(problem).name, "problem");
        assert_eq!(f.local(problem).ty, Type::Text);
        assert_eq!(f.block(pad).label, "attempt catch");
        // The failing function carries the Fail capability.
        let divf = m
            .items
            .iter()
            .find_map(|i| match i {
                MirItem::Function(f) if f.name == "divide" => Some(f),
                _ => None,
            })
            .expect("the divide function");
        assert!(divf.can_fail);
        assert_eq!(divf.capabilities, vec![Capability::Fail]);
    }

    #[test]
    fn fail_targets_innermost_pad() {
        let m = build(
            "function risky business\n    can fail\n    fail with \"boom\"\nattempt risky business if it fails then\n    say \"caught\"\notherwise\n    say \"no\"",
        );
        // The caller's main has one pad; the failing function's Fail has none
        // inside itself.
        let f = main_of(&m);
        assert_eq!(f.catch_pads.len(), 1);
        let rf = m
            .items
            .iter()
            .find_map(|i| match i {
                MirItem::Function(f) if f.name == "risky business" => Some(f),
                _ => None,
            })
            .expect("the failing function");
        let fail = rf
            .blocks
            .iter()
            .find(|b| matches!(b.term, Term::Fail { .. }))
            .expect("fail with is a Fail");
        match &fail.term {
            Term::Fail { catch, .. } => assert!(catch.is_none(), "top-level fail propagates out"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn propagate_reaches_the_outer_pad() {
        // `inner step` fails; `outer step` catches it with its own pad; the
        // script body has no attempt at all.
        let m = build(
            "function inner step\n    can fail\n    fail with \"x\"\nfunction outer step\n    attempt inner step if it fails then\n        say \"zero\"\n    otherwise\n        say \"one\"\nouter step\nsay \"done\"",
        );
        let outerf = m
            .items
            .iter()
            .find_map(|i| match i {
                MirItem::Function(f) if f.name == "outer step" => Some(f),
                _ => None,
            })
            .expect("outer step");
        assert_eq!(outerf.catch_pads.len(), 1, "outer's attempt makes one pad");
        let f = main_of(&m);
        assert_eq!(f.catch_pads.len(), 0, "main has no attempt here");
    }

    #[test]
    fn as_tail_binds_the_chosen_name() {
        let m = build(
            "function divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    give back top divided by bottom\nattempt divide 10 and 0 as problem\n    say problem\notherwise\n    say result",
        );
        let f = main_of(&m);
        assert!(f.locals.iter().any(|l| l.name == "problem" && l.ty == Type::Text));
        // `result` is a scope alias for the attempt's success value — the
        // call-result local — not a separate local. The otherwise branch
        // (the "attempt success" block) must `say` that local.
        let succ = f
            .blocks
            .iter()
            .find(|b| b.label == "attempt success")
            .expect("success block");
        let says = succ
            .instrs
            .iter()
            .filter_map(|i| match i {
                Instr::Say { value, .. } => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(says.len(), 1);
        let said = says[0].as_local().expect("say reads a local");
        assert_eq!(f.local_ty(said), Type::Decimal, "the call's success value");
    }

    // ----- LOM: the pass, the ring, the report -----

    #[test]
    fn dev_instrumentation_adds_probes_release_adds_none() {
        let src = "make score equal to 5\nsay score";
        let (ast, _) = parse(src);
        let h = hir::lower(check(&ast, src));

        let mut dev = lower(h.clone());
        let dev_added = instrument(&mut dev, LomConfig::DEV);
        assert!(dev_added >= 1, "dev builds emit bind probes");
        assert_eq!(count_events(&dev), dev_added);

        let mut rel = lower(h);
        let rel_added = instrument(&mut rel, LomConfig::RELEASE);
        assert_eq!(rel_added, 0);
        assert_eq!(count_events(&rel), 0, "release builds are silent (26.5)");
    }

    fn count_events(prog: &MirProgram) -> usize {
        prog.items
            .iter()
            .map(|item| {
                let f = match item {
                    MirItem::Main(f) | MirItem::Function(f) | MirItem::Test(f) => f,
                };
                f.blocks
                    .iter()
                    .map(|b| b.instrs.iter().filter(|i| is_event(i)).count())
                    .sum::<usize>()
            })
            .sum()
    }

    #[test]
    fn function_entry_probe_snapshots_params() {
        let mut prog = build(
            "function greet well\n    takes text called name\n    say name\ngreet well \"bo\"",
        );
        let added = instrument(&mut prog, LomConfig::DEV);
        assert!(added >= 1);
        let g = prog
            .items
            .iter()
            .find_map(|i| match i {
                MirItem::Function(f) if f.name == "greet well" => Some(f),
                _ => None,
            })
            .unwrap();
        let entry = &g.blocks[0];
        assert!(matches!(
            &entry.instrs[0],
            Instr::EventFunctionEntry { function, args }
                if function == "greet well" && args.len() == 1 && args[0].0 == "name"
        ));
    }

    #[test]
    fn ring_is_bounded_and_evicts_oldest() {
        let mut ring = EventRing::new(2);
        ring.push(LomEvent::Bind { name: "a".into(), value: "1".into(), site: 0 });
        ring.push(LomEvent::Bind { name: "b".into(), value: "2".into(), site: 0 });
        ring.push(LomEvent::Bind { name: "c".into(), value: "3".into(), site: 0 });
        assert_eq!(ring.len(), 2);
        assert!(matches!(ring.events()[0], LomEvent::Bind { ref name, .. } if name == "b"));
    }

    #[test]
    fn failure_report_names_values_and_lines() {
        let src = "make bottom equal to 0\nsay bottom";
        let mut ring = EventRing::new(16);
        ring.push(LomEvent::Bind { name: "bottom".into(), value: "0".into(), site: 5 });
        ring.push(LomEvent::Failure { message: "divided by zero".into() });
        let report = failure_report(&ring, "divide", src);
        assert!(report.contains("`bottom` became 0"), "{report}");
        assert!(report.contains("line 1"), "{report}");
        assert!(report.contains("divided by zero"), "{report}");
        assert!(report.contains("in `divide`"), "{report}");
    }

    #[test]
    fn empty_ring_report_is_honest() {
        let ring = EventRing::new(16);
        let report = failure_report(&ring, "main", "say 1");
        assert!(report.contains("no recorded events"), "{report}");
    }

    // ----- verifier -----

    #[test]
    fn verifier_rejects_open_blocks() {
        // All `build` fixtures already ran `verify`; prove the negative too.
        let mut prog = build("say 1");
        let f = match &mut prog.items[0] {
            MirItem::Main(f) => f,
            _ => unreachable!(),
        };
        f.blocks[0].term = Term::Unreachable;
        assert!(verify(&prog).is_err());
    }

    #[test]
    fn multiple_statements_lower_in_order() {
        let m = build("say 1\nsay 2\nsay 3");
        let f = main_of(&m);
        let says = instrs(f).iter().filter(|i| matches!(i, Instr::Say { .. })).count();
        assert_eq!(says, 3);
        assert_eq!(f.blocks.len(), 1, "straight-line code stays one block");
    }
}
