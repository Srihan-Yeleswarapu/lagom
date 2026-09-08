//! The Lagom HIR (desugared, still high-level) — M0 subset.
//!
//! Spec anchor: 00 §22.1, §21.1 (pipeline stage 5), doc 08 §2 (the HIR
//! span-retention contract). All sugar eliminated:
//!
//! - clauses → typed signatures (HIR is the canonical holder for backends),
//! - flowing reads/calls → ordinary call/field/index nodes (sema's rebuilt
//!   calls are classified here: reads were rebuilt as `Call`s with one
//!   prepositional argument; HIR turns them back into `Field`/`Index` nodes),
//! - string interpolation → a `format`-parts node (S-9's printing applies at
//!   runtime, in one place — no per-type special cases),
//! - `make`/`set`/`increase`/`decrease` → binding/mutation statements
//!   (`increase`/`decrease` keep the one-mutation-form invariant by MIR),
//! - operator words → operator nodes (7.3's ladder, already flat),
//! - `of type` annotations → checked-and-dropped (the binding's type is the
//!   contract; the annotation lives in the checked type),
//! - option narrowing is *not* HIR work: M0 sees it as the ordinary
//!   comparison `x is equal to nothing` (8.5's narrowing is enforced by
//!   sema's comparability rules; M1 adds flow typing as its own pass).
//!
//! **Span retention (doc 08 §2): every node keeps the span of the source it
//! came from, including desugared pieces.** Static types ride on every
//! expression node, read from sema's per-node type table (keyed by span) —
//! the Cranelift backend needs per-node types; the interpreter can dispatch
//! on values but reads the same table for provenance.

use std::collections::{HashMap, HashSet};

use lagom_diagnostics::Span;
use lagom_sema::{
    CheckedAttemptTail, CheckedItem, CheckedProgram, CheckedStmt, CheckedStmtKind, Type, TypedExpr,
};

// ---------------------------------------------------------------------------
// Program structure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HirProgram {
    pub items: Vec<HirItem>,
}

#[derive(Debug, Clone)]
pub enum HirItem {
    Use { module: String, span: Span },
    Function(HirFunction),
    Structure(HirStructure),
    /// A test body is a zero-parameter function (`lagom test` calls each).
    Test(HirFunction),
    /// A top-level statement. The driver concatenates these into the program
    /// entry at M0 (§19.1's script model).
    Main(HirStmt),
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<(String, Type, Span)>,
    pub ret: Type,
    pub can_fail: bool,
    /// False = inferred from the body's `give back` (7.8's optional `returns`).
    pub ret_declared: bool,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirStructure {
    pub name: String,
    /// (name, type, decl span) in declaration order.
    pub fields: Vec<(String, Type, Span)>,
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HirStmt {
    pub kind: HirStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind {
    /// `make`/`make changing` — a binding.
    Bind {
        mutable: bool,
        name: String,
        name_span: Span,
        value: HirExpr,
    },
    /// `set <target> to <value>`.
    Assign {
        target: HirPlace,
        value: HirExpr,
    },
    /// `increase`/`decrease` — kept as a distinct op through MIR (the runtime
    /// needs the read-modify-write origin for LOM events and overflow traps).
    AssignOp {
        target: HirPlace,
        /// true = add (`increase`), false = subtract (`decrease`).
        increase: bool,
        value: HirExpr,
    },
    If {
        branches: Vec<(HirExpr, Vec<HirStmt>, Span)>,
        otherwise: Option<Vec<HirStmt>>,
    },
    /// `repeat 10 times using i` — MIR lowers to init/test/inc.
    RepeatCount {
        times: i64,
        times_span: Span,
        binding: Option<(String, Span)>,
        body: Vec<HirStmt>,
    },
    RepeatWhile {
        cond: HirExpr,
        body: Vec<HirStmt>,
    },
    RepeatForEach {
        item: (String, Span),
        index: Option<(String, Span)>,
        iter: HirExpr,
        body: Vec<HirStmt>,
    },
    Stop,
    Next,
    Return {
        value: HirExpr,
    },
    /// `fail with <text>` — the failure form (13.1).
    Fail {
        message: HirExpr,
    },
    /// `attempt <expr> [tail]` — the capability boundary, statement form.
    Attempt {
        inner: HirExpr,
        tail: Option<HirAttemptTail>,
    },
    /// `check that <expr>` — a test assertion (5.1); the runtime renders the
    /// comparison on failure (docs/13 G-4).
    /// `check that <expr>` — when the expression is a comparison, its
    /// operator and operand expressions ride along (cloned) so the runtime
    /// can render doc 13's G-4 failure format naming both sides.
    Check {
        expr: HirExpr,
        cmp: Option<(BinOp, Box<HirExpr>, Box<HirExpr>)>,
    },
    /// An expression evaluated for effect (`say …`, a bare call).
    Effect(HirExpr),
}

#[derive(Debug, Clone)]
pub enum HirAttemptTail {
    /// `and pass the problem on`.
    Propagate,
    /// `if it fails then … otherwise …` — binds `problem` / `result` (13.1).
    IfItFails {
        then_block: Vec<HirStmt>,
        otherwise: Option<Vec<HirStmt>>,
    },
    /// `as <name> …` — binds the error under a chosen name.
    As {
        name: String,
        name_span: Span,
        block: Vec<HirStmt>,
        otherwise: Option<Vec<HirStmt>>,
    },
}

// ---------------------------------------------------------------------------
// Places (assignment targets)
// ---------------------------------------------------------------------------

/// An assignment target: a local, or a path through containers/structs.
/// HIR keeps the path; MIR lowers to explicit load/store sequences.
#[derive(Debug, Clone)]
pub struct HirPlace {
    pub base: String,
    pub base_span: Span,
    /// Accessor path in source order.
    pub path: Vec<HirAccess>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirAccess {
    /// `at <expr>` — list index or map key.
    Index(HirExpr),
    /// `of <field>` — struct field.
    Field(String, Span),
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    /// The option sentinel (8.5).
    Nothing,
    /// A local binding read.
    Local(String),
    /// An ordinary call: user function or builtin, arguments in call order.
    /// All sugar (flowing form, `and`-args, `with`-labels) is gone.
    Call {
        callee: String,
        callee_span: Span,
        args: Vec<HirExpr>,
    },
    /// A flowing field read: `name of p`.
    Field {
        base: Box<HirExpr>,
        field: String,
        field_span: Span,
    },
    /// A flowing index read: `things at 2`.
    Index {
        base: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    /// Struct construction: `a player with name "bo" and score 0` — fields in
    /// declaration order (7.11 makes construction order-insensitive; the
    /// checker validated completeness).
    StructLit {
        name: String,
        fields: Vec<(String, HirExpr, Span)>,
    },
    List(Vec<HirExpr>),
    Map(Vec<(HirExpr, HirExpr)>),
    Pair(Box<HirExpr>, Box<HirExpr>),
    Unary {
        op: UnOp,
        inner: Box<HirExpr>,
    },
    Binary {
        op: BinOp,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
    },
    /// Interpolation desugared to format-parts (7.7): literal chunks plus
    /// operand expressions. The runtime's `format` applies S-9's per-type
    /// printing in one place.
    Format {
        parts: Vec<HirFormatPart>,
    },
    /// A bare attempt in expression position (13.1/R-20.1): evaluates to the
    /// success value; propagation is the enclosing function's `can fail`.
    Attempt(Box<HirExpr>),
}

#[derive(Debug, Clone)]
pub enum HirFormatPart {
    Lit(String),
    /// An interpolated expression (the LOM provenance names it as "the
    /// `{total}` in this text").
    Value(HirExpr),
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

// ---------------------------------------------------------------------------
// Lowering
// ---------------------------------------------------------------------------

/// Lower a checked program to HIR.
pub fn lower(program: CheckedProgram) -> HirProgram {
    let LowerTables { types, callee_spans, reads } = LowerTables {
        types: program.node_types,
        callee_spans: program.callee_spans,
        reads: program.read_spans,
    };
    let mut lx = Lowerer { types: &types, callee_spans: &callee_spans, reads: &reads };
    let mut items = Vec::new();
    for item in program.items {
        match item {
            CheckedItem::Use { module, span } => items.push(HirItem::Use { module, span }),
            CheckedItem::Function(f) => items.push(HirItem::Function(lx.function(f))),
            CheckedItem::Structure(s) => items.push(HirItem::Structure(HirStructure {
                name: s.name,
                fields: s.fields,
            })),
            CheckedItem::Test(t) => items.push(HirItem::Test(lx.function(t))),
            CheckedItem::Stmt(s) => items.push(HirItem::Main(lx.stmt(s))),
        }
    }
    HirProgram { items }
}

/// Owned side tables from sema; the `Lowerer` borrows them.
struct LowerTables {
    types: HashMap<Span, Type>,
    callee_spans: HashMap<Span, Span>,
    reads: HashSet<Span>,
}

struct Lowerer<'t> {
    types: &'t HashMap<Span, Type>,
    callee_spans: &'t HashMap<Span, Span>,
    reads: &'t HashSet<Span>,
}

impl<'t> Lowerer<'t> {
    /// The checked type of the node at `span`. The type table covers every
    /// node sema checked; interpolation bodies re-lex with local spans, so
    /// those children may be absent — they fall back to the node default
    /// (`Error`), which the interpreter handles and the backend never sees in
    /// a clean program.
    fn ty(&self, span: Span, fallback: Type) -> Type {
        self.types.get(&span).cloned().unwrap_or(fallback)
    }

    fn function(&mut self, f: lagom_sema::CheckedFunction) -> HirFunction {
        let ret_declared = f.returns.is_some();
        let body: Vec<HirStmt> = f.body.into_iter().map(|s| self.stmt(s)).collect();
        HirFunction {
            name: f.name,
            name_span: f.name_span,
            params: f.params.into_iter().map(|p| (p.name, p.ty, p.span)).collect(),
            ret: f.returns.unwrap_or(Type::Number),
            can_fail: f.can_fail,
            ret_declared,
            body,
            span: f.span,
        }
    }

    fn stmt(&mut self, s: CheckedStmt) -> HirStmt {
        let span = s.span;
        let kind = match s.kind {
            CheckedStmtKind::Make { mutable, name, value } => HirStmtKind::Bind {
                mutable,
                name_span: span,
                name,
                value: self.expr(value),
            },
            CheckedStmtKind::Set { target, value } => HirStmtKind::Assign {
                target: self.target(target),
                value: self.expr(value),
            },
            CheckedStmtKind::Change { decrease, target, value } => HirStmtKind::AssignOp {
                target: self.target(target),
                increase: !decrease,
                value: self.expr(value),
            },
            CheckedStmtKind::If { branches, otherwise } => HirStmtKind::If {
                branches: branches
                    .into_iter()
                    .map(|(c, b)| {
                        let cspan = c.span();
                        (self.expr(c), b.into_iter().map(|s| self.stmt(s)).collect(), cspan)
                    })
                    .collect(),
                otherwise: otherwise.map(|b| b.into_iter().map(|s| self.stmt(s)).collect()),
            },
            CheckedStmtKind::RepeatCount { times, binding, body } => HirStmtKind::RepeatCount {
                times,
                times_span: span,
                binding: binding.map(|b| (b, span)),
                body: body.into_iter().map(|s| self.stmt(s)).collect(),
            },
            CheckedStmtKind::RepeatWhile { cond, body } => HirStmtKind::RepeatWhile {
                cond: self.expr(cond),
                body: body.into_iter().map(|s| self.stmt(s)).collect(),
            },
            CheckedStmtKind::RepeatForEach { item, index, iter, body } => {
                let ispan = iter.span();
                let mut lowered = self.expr(iter);
                lowered.span = ispan;
                HirStmtKind::RepeatForEach {
                    item: (item, span),
                    index: index.map(|i| (i, span)),
                    iter: lowered,
                    body: body.into_iter().map(|s| self.stmt(s)).collect(),
                }
            }
            CheckedStmtKind::Stop => HirStmtKind::Stop,
            CheckedStmtKind::Next => HirStmtKind::Next,
            CheckedStmtKind::GiveBack { value } => HirStmtKind::Return {
                value: self.expr(value),
            },
            CheckedStmtKind::FailWith { value } => HirStmtKind::Fail {
                message: self.expr(value),
            },
            CheckedStmtKind::Attempt { inner, tail } => HirStmtKind::Attempt {
                inner: self.expr(inner),
                tail: tail.map(|t| self.tail(t)),
            },
            CheckedStmtKind::CheckThat { expr } => {
                // Extract the comparison shape (G-4) before consuming expr:
                // when the checked expression IS a comparison, lower it once
                // and reuse the two operand nodes for both the boolean value
                // and the failure report's sides.
                let cmp_shape = match &expr.expr {
                    lagom_ast::Expr::Binary { op, left, right, .. } => {
                        match op {
                            lagom_ast::BinOp::Equal
                            | lagom_ast::BinOp::NotEqual
                            | lagom_ast::BinOp::Greater
                            | lagom_ast::BinOp::Less
                            | lagom_ast::BinOp::AtLeast
                            | lagom_ast::BinOp::AtMost => Some((*op, left, right)),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                match cmp_shape {
                    Some((op, left, right)) => {
                        let lspan = lagom_sema::expr_span(left);
                        let rspan = lagom_sema::expr_span(right);
                        let lty = self.ty(lspan, Type::Error);
                        let rty = self.ty(rspan, Type::Error);
                        let l = self.expr(TypedExpr { expr: (**left).clone(), ty: lty });
                        let r = self.expr(TypedExpr { expr: (**right).clone(), ty: rty });
                        let bop = lower_binop(op);
                        let value = HirExpr {
                            kind: HirExprKind::Binary {
                                op: bop,
                                left: Box::new(l.clone()),
                                right: Box::new(r.clone()),
                            },
                            ty: Type::Boolean,
                            span: expr.span(),
                        };
                        HirStmtKind::Check {
                            expr: value,
                            cmp: Some((bop, Box::new(l), Box::new(r))),
                        }
                    }
                    None => HirStmtKind::Check {
                        expr: self.expr(expr),
                        cmp: None,
                    },
                }
            }
            CheckedStmtKind::ExprStmt { expr } => HirStmtKind::Effect(self.expr(expr)),
        };
        HirStmt { kind, span }
    }

    fn tail(&mut self, t: CheckedAttemptTail) -> HirAttemptTail {
        match t {
            CheckedAttemptTail::Propagate => HirAttemptTail::Propagate,
            CheckedAttemptTail::IfItFails { then_block, otherwise } => HirAttemptTail::IfItFails {
                then_block: then_block.into_iter().map(|s| self.stmt(s)).collect(),
                otherwise: otherwise.map(|b| b.into_iter().map(|s| self.stmt(s)).collect()),
            },
            CheckedAttemptTail::As { name, name_span, block, otherwise } => HirAttemptTail::As {
                name,
                name_span,
                block: block.into_iter().map(|s| self.stmt(s)).collect(),
                otherwise: otherwise.map(|b| b.into_iter().map(|s| self.stmt(s)).collect()),
            },
        }
    }

    fn target(&mut self, t: lagom_sema::TypedTarget) -> HirPlace {
        HirPlace {
            base: t.base,
            base_span: t.span,
            path: t
                .accessors
                .into_iter()
                .map(|a| match a {
                    lagom_sema::TypedAccessor::At { index } => {
                        let ispan = index.span();
                        let mut e = self.expr(index);
                        e.span = ispan;
                        HirAccess::Index(e)
                    }
                    lagom_sema::TypedAccessor::Of { field } => {
                        HirAccess::Field(field, t.span)
                    }
                })
                .collect(),
            span: t.span,
        }
    }

    fn expr(&mut self, e: TypedExpr) -> HirExpr {
        let span = e.span();
        let ty = self.ty(span, e.ty.clone());
        let kind = self.expr_kind(e);
        HirExpr { kind, ty, span }
    }

    fn expr_kind(&mut self, e: TypedExpr) -> HirExprKind {
        match e.expr {
            lagom_ast::Expr::Int { value, .. } => HirExprKind::Int(value),
            lagom_ast::Expr::Float { value, .. } => HirExprKind::Float(value),
            lagom_ast::Expr::Text { value, .. } => HirExprKind::Text(value),
            lagom_ast::Expr::Bool { value, .. } => HirExprKind::Bool(value),
            lagom_ast::Expr::Nothing { .. } => HirExprKind::Nothing,
            lagom_ast::Expr::Name { name, .. } => HirExprKind::Local(name.display()),
            lagom_ast::Expr::Interp { parts, .. } => HirExprKind::Format {
                parts: parts
                    .into_iter()
                    .map(|p| match p {
                        lagom_ast::InterpPart::Lit(s) => HirFormatPart::Lit(s),
                        lagom_ast::InterpPart::Expr(inner) => {
                            let ispan = lagom_sema::expr_span(&inner);
                            let ity = self.ty(ispan, Type::Error);
                            let mut lowered = self.expr(TypedExpr { expr: inner, ty: ity.clone() });
                            lowered.ty = ity;
                            HirFormatPart::Value(lowered)
                        }
                    })
                    .collect(),
            },
            lagom_ast::Expr::Group { inner, .. } => {
                // Parenthesization is not semantic; the inner expression is
                // lowered with the group's own span preserved on the child.
                let ispan = lagom_sema::expr_span(&inner);
                let ity = self.ty(ispan, Type::Error);
                let mut lowered = self.expr(TypedExpr { expr: *inner, ty: ity });
                lowered.span = ispan;
                lowered.kind
            }
            lagom_ast::Expr::Neg { inner, .. } => {
                let ispan = lagom_sema::expr_span(&inner);
                let ity = self.ty(ispan, Type::Error);
                let mut lowered = self.expr(TypedExpr { expr: *inner, ty: ity });
                lowered.span = ispan;
                HirExprKind::Unary {
                    op: UnOp::Neg,
                    inner: Box::new(lowered),
                }
            }
            lagom_ast::Expr::Not { inner, .. } => {
                let ispan = lagom_sema::expr_span(&inner);
                let ity = self.ty(ispan, Type::Boolean);
                let mut lowered = self.expr(TypedExpr { expr: *inner, ty: ity });
                lowered.span = ispan;
                HirExprKind::Unary {
                    op: UnOp::Not,
                    inner: Box::new(lowered),
                }
            }
            lagom_ast::Expr::Binary { op, left, right, .. } => {
                let lspan = lagom_sema::expr_span(&left);
                let rspan = lagom_sema::expr_span(&right);
                let lty = self.ty(lspan, Type::Error);
                let rty = self.ty(rspan, Type::Error);
                let mut l = self.expr(TypedExpr { expr: *left, ty: lty });
                let mut r = self.expr(TypedExpr { expr: *right, ty: rty });
                l.span = lspan;
                r.span = rspan;
                HirExprKind::Binary {
                    op: lower_binop(op),
                    left: Box::new(l),
                    right: Box::new(r),
                }
            }
            lagom_ast::Expr::ListLit { elements, .. } => HirExprKind::List(
                elements
                    .into_iter()
                    .map(|el| {
                        let es = lagom_sema::expr_span(&el);
                        let ety = self.ty(es, Type::Error);
                        let mut lowered = self.expr(TypedExpr { expr: el, ty: ety });
                        lowered.span = es;
                        lowered
                    })
                    .collect(),
            ),
            lagom_ast::Expr::MapLit { entries, .. } => HirExprKind::Map(
                entries
                    .into_iter()
                    .map(|(k, v)| {
                        let ks = lagom_sema::expr_span(&k);
                        let vs = lagom_sema::expr_span(&v);
                        let kty = self.ty(ks, Type::Error);
                        let vty = self.ty(vs, Type::Error);
                        let mut lk = self.expr(TypedExpr { expr: k, ty: kty });
                        let mut lv = self.expr(TypedExpr { expr: v, ty: vty });
                        lk.span = ks;
                        lv.span = vs;
                        (lk, lv)
                    })
                    .collect(),
            ),
            lagom_ast::Expr::PairLit { first, second, .. } => {
                let fs = lagom_sema::expr_span(&first);
                let ss = lagom_sema::expr_span(&second);
                let fty = self.ty(fs, Type::Error);
                let sty = self.ty(ss, Type::Error);
                let mut f = self.expr(TypedExpr { expr: *first, ty: fty });
                let mut s = self.expr(TypedExpr { expr: *second, ty: sty });
                f.span = fs;
                s.span = ss;
                HirExprKind::Pair(Box::new(f), Box::new(s))
            }
            lagom_ast::Expr::StructLit { name, fields, .. } => HirExprKind::StructLit {
                name: name.display(),
                fields: fields
                    .into_iter()
                    .map(|(n, v)| {
                        let vs = lagom_sema::expr_span(&v);
                        let vty = self.ty(vs, Type::Error);
                        let mut lowered = self.expr(TypedExpr { expr: v, ty: vty });
                        lowered.span = vs;
                        (n.display(), lowered, n.span)
                    })
                    .collect(),
            },
            lagom_ast::Expr::AttemptExpr { expr, .. } => {
                let ispan = lagom_sema::expr_span(&expr);
                let ity = self.ty(ispan, Type::Error);
                let mut lowered = self.expr(TypedExpr { expr: *expr, ty: ity });
                lowered.span = ispan;
                HirExprKind::Attempt(Box::new(lowered))
            }
            lagom_ast::Expr::Call(call) => self.call(*call),
        }
    }

    /// Classify a resolved call. Sema tags flowing reads precisely
    /// (`read_spans`): `name of p` becomes `Field`, `things at 2` becomes
    /// `Index`; everything else is an ordinary `Call`, args in call order.
    fn call(&mut self, call: lagom_ast::CallExpr) -> HirExprKind {
        let callee = call.callee.display();
        let callee_span = *self
            .callee_spans
            .get(&call.span)
            .unwrap_or(&call.callee.span);
        let is_read = self.reads.contains(&call.span);
        if is_read {
            if let Some((lagom_ast::Prep::Of, a)) = call.preps.first() {
                let base_ty = self.ty(a.span, Type::Error);
                let mut base = self.expr(TypedExpr { expr: *a.expr.clone(), ty: base_ty });
                base.span = a.span;
                return HirExprKind::Field {
                    base: Box::new(base),
                    field: callee,
                    field_span: callee_span,
                };
            }
            if let Some(at) = call.preps.iter().find(|(p, _)| *p == lagom_ast::Prep::At) {
                // `at` read: the callee is the base name, the `at` argument
                // the index (`things at 2`). The parser puts prepositional
                // arguments in `preps`, not `first`.
                let base_ty = self.ty(call.callee.span, Type::Error);
                let base = HirExpr {
                    kind: HirExprKind::Local(callee.clone()),
                    ty: base_ty.clone(),
                    span: call.callee.span,
                };
                let idx_ty = self.ty(at.1.span, Type::Error);
                let mut index = self.expr(TypedExpr { expr: (*at.1.expr).clone(), ty: idx_ty });
                index.span = at.1.span;
                return HirExprKind::Index {
                    base: Box::new(base),
                    index: Box::new(index),
                };
            }
        }
        // Ordinary call: gather args in call order.
        let mut args: Vec<lagom_ast::Expr> = Vec::new();
        if let Some(first) = call.first {
            args.push(*first.expr);
        }
        for (_, a) in call.preps {
            args.push(*a.expr);
        }
        for a in call.and_args {
            args.push(*a.expr);
        }
        for (_, a) in call.with_args {
            args.push(*a.expr);
        }
        let lowered: Vec<HirExpr> = args
            .into_iter()
            .map(|a| {
                let aspan = lagom_sema::expr_span(&a);
                let aty = self.ty(aspan, Type::Error);
                let mut l = self.expr(TypedExpr { expr: a, ty: aty });
                l.span = aspan;
                l
            })
            .collect();
        HirExprKind::Call { callee, callee_span, args: lowered }
    }
}

fn lower_binop(op: lagom_ast::BinOp) -> BinOp {
    use lagom_ast::BinOp::*;
    match op {
        Add => BinOp::Add,
        Sub => BinOp::Sub,
        Mul => BinOp::Mul,
        Div => BinOp::Div,
        DivEvenly => BinOp::DivEvenly,
        Rem => BinOp::Rem,
        And => BinOp::And,
        Or => BinOp::Or,
        Equal => BinOp::Equal,
        NotEqual => BinOp::NotEqual,
        Greater => BinOp::Greater,
        Less => BinOp::Less,
        AtLeast => BinOp::AtLeast,
        AtMost => BinOp::AtMost,
    }
}
