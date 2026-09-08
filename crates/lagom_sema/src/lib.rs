//! The Lagom semantic analyzer (M0 subset).
//!
//! Spec anchors: 00 §7.0.3 (greedy multi-word names), §7.9 (R-4 call grammar),
//! §8.1–8.5 (D-10/D-11 inference-first, R-15 literal typing, D-30 equality,
//! 8.5 options), §13.1 (fail-capability model), docs/13 §3 (S-1…S-13),
//! docs/14 G-9/G-10/G-11.
//!
//! Division of labor (frozen): the parser builds the syntax tree but does NOT
//! resolve multi-word names — §7.0.3's greedy longest-match needs the *bound
//! names in scope*, which only sema knows. Names arrive as word runs and are
//! resolved here:
//!
//! - **Name resolution** (§7.0.3): word runs resolve greedily against bindings
//!   + signatures in scope; ambiguity between two bound names where one is a
//!   prefix of the other is rejected at declaration time, never at use time.
//! - **Type checking** (§8, S-1/S-3/S-4): D-10 numbers are i64, decimals f64;
//!   `divided by` promotes; mixed arithmetic promotes to decimal (the only
//!   implicit conversion); literals are polymorphic at inference (R-15) —
//!   they unify with the expected type and default to `number`.
//! - **Mutability** (§7.2): `set`/`increase`/`decrease` and element/field
//!   writes require a `changing` binding.
//! - **Options** (§8.5, S-13): `first of list` is `T?`; `is nothing` /
//!   `is something` compare against options; `nothing` never compares against
//!   a plain value (no null exists to compare with — D-30 family).
//! - **The capability check** (§13.1, S-6): every `can fail` call site must be
//!   `attempt`-wrapped; an unhandled one is a compile error (Rust/Swift rule).
//!
//! Every diagnostic carries a code and a source span.

use std::collections::{HashMap, HashSet};

use lagom_ast as ast;
use lagom_ast::{Accessor, BinOp, CallExpr, Expr, Name, Prep, Program, Repeat, Stmt, TypeExpr};
use lagom_diagnostics::{Diagnostic, Diagnostics, Span};

// ---------------------------------------------------------------------------
// Public surface — what HIR/MIR/codegen consume
// ---------------------------------------------------------------------------

pub struct CheckedProgram {
    pub items: Vec<CheckedItem>,
    /// All diagnostics produced while checking (empty when clean).
    pub diags: Diagnostics,
    /// Per-node types keyed by source span — the side table HIR reads to
    /// type every desugared node (spans are unique per checked node except
    /// the parser-synthesized `is nothing` pieces, which HIR self-types).
    pub node_types: HashMap<Span, Type>,
    /// Call span → callee-name span (LOM provenance anchors, 26.5).
    pub callee_spans: HashMap<Span, Span>,
    /// Call spans that are flowing reads (HIR classifies them Field/Index).
    pub read_spans: HashSet<Span>,
}

#[derive(Debug)]
pub enum CheckedItem {
    Use { module: String, span: Span },
    Function(CheckedFunction),
    Structure(CheckedStructure),
    Test(CheckedFunction),
    Stmt(CheckedStmt),
}

#[derive(Debug)]
pub struct CheckedFunction {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<CheckedParam>,
    pub returns: Option<Type>,
    pub can_fail: bool,
    pub body: Vec<CheckedStmt>,
    pub span: Span,
}

#[derive(Debug)]
pub struct CheckedParam {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CheckedStructure {
    pub name: String,
    /// Fields in declaration order: (name, type, decl span).
    pub fields: Vec<(String, Type, Span)>,
}

#[derive(Debug, Clone)]
pub struct CheckedStmt {
    pub kind: CheckedStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CheckedStmtKind {
    Make {
        mutable: bool,
        name: String,
        value: TypedExpr,
    },
    Set {
        target: TypedTarget,
        value: TypedExpr,
    },
    Change {
        decrease: bool,
        target: TypedTarget,
        value: TypedExpr,
    },
    If {
        branches: Vec<(TypedExpr, Vec<CheckedStmt>)>,
        otherwise: Option<Vec<CheckedStmt>>,
    },
    RepeatCount {
        times: i64,
        binding: Option<String>,
        body: Vec<CheckedStmt>,
    },
    RepeatWhile {
        cond: TypedExpr,
        body: Vec<CheckedStmt>,
    },
    RepeatForEach {
        item: String,
        index: Option<String>,
        iter: TypedExpr,
        body: Vec<CheckedStmt>,
    },
    Stop,
    Next,
    GiveBack {
        value: TypedExpr,
    },
    FailWith {
        value: TypedExpr,
    },
    Attempt {
        inner: TypedExpr,
        tail: Option<CheckedAttemptTail>,
    },
    CheckThat {
        expr: TypedExpr,
    },
    ExprStmt {
        expr: TypedExpr,
    },
}

#[derive(Debug, Clone)]
pub enum CheckedAttemptTail {
    Propagate,
    IfItFails {
        then_block: Vec<CheckedStmt>,
        otherwise: Option<Vec<CheckedStmt>>,
    },
    As {
        name: String,
        name_span: Span,
        block: Vec<CheckedStmt>,
        otherwise: Option<Vec<CheckedStmt>>,
    },
}

#[derive(Debug, Clone)]
pub struct TypedTarget {
    pub base: String,
    pub accessors: Vec<TypedAccessor>,
    /// The type *after* all accessors — the assignment's target type.
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedAccessor {
    At { index: TypedExpr },
    Of { field: String },
}

/// An expression with its checked type. The AST `Expr` is carried as-is
/// (M0's HIR desugars it; sema annotates rather than rewrites — except that
/// greedy §7.0.3 splits are applied to the stored `Expr` itself, so downstream
/// stages see resolved calls).
#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub expr: Expr,
    pub ty: Type,
}

impl TypedExpr {
    pub fn span(&self) -> Span {
        expr_span(&self.expr)
    }
}

// ---------------------------------------------------------------------------
// Types (D-10, S-1/S-3; docs/13 §3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Decimal,
    Text,
    Boolean,
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Pair(Box<Type>, Box<Type>),
    /// `T?` — §8.5. `first of list` returns `T?` (S-13).
    Option(Box<Type>),
    /// A user structure.
    Struct(String),
    /// The failure value's type at student level (S-10: a text message).
    Failure,
    /// A numeric literal before unification (R-15): compatible with Number and
    /// Decimal, defaults to Number.
    NumericLit,
    /// The type of a bare `nothing` (§8.5) — only comparable against options.
    NothingLit,
    /// Recovered/unknown after a diagnostic.
    Error,
}

impl Type {
    /// Word-style rendering for diagnostics (7.10).
    pub fn display(&self) -> String {
        match self {
            Type::Number => "number".into(),
            Type::Decimal => "decimal".into(),
            Type::Text => "text".into(),
            Type::Boolean => "boolean".into(),
            Type::List(t) => format!("a list of {}", t.display()),
            Type::Map(k, v) => format!("a map from {} to {}", k.display(), v.display()),
            Type::Pair(a, b) => format!("a pair of {} and {}", a.display(), b.display()),
            Type::Option(t) => format!("a {} or nothing", t.display()),
            Type::Struct(n) => n.clone(),
            Type::Failure => "a failure value".into(),
            Type::NumericLit => "number".into(),
            Type::NothingLit => "nothing".into(),
            Type::Error => "an unknown type (because of an earlier error)".into(),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Type::Number | Type::Decimal | Type::NumericLit)
    }
}

impl From<&TypeExpr> for Type {
    fn from(t: &TypeExpr) -> Self {
        match t {
            TypeExpr::Number => Type::Number,
            TypeExpr::Decimal => Type::Decimal,
            TypeExpr::Text => Type::Text,
            TypeExpr::Boolean => Type::Boolean,
            TypeExpr::List(e) => Type::List(Box::new(e.as_ref().into())),
            TypeExpr::Map(k, v) => Type::Map(Box::new(k.as_ref().into()), Box::new(v.as_ref().into())),
            TypeExpr::Pair(a, b) => Type::Pair(Box::new(a.as_ref().into()), Box::new(b.as_ref().into())),
            TypeExpr::User(n) => Type::Struct(n.display()),
            TypeExpr::Inferred => Type::Error,
        }
    }
}

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Callable {
    params: Vec<Type>,
    ret: Type,
    can_fail: bool,
    origin: Origin,
}

#[derive(Clone, Copy, PartialEq)]
enum Origin {
    User,
    Standard,
    Math,
}

#[derive(Clone)]
struct Binding {
    ty: Type,
    mutable: bool,
    span: Span,
}

struct Scope {
    bindings: HashMap<String, Binding>,
    order: Vec<String>,
}

/// What kind of body is being checked (governs `give back` rules).
#[derive(Clone, Copy, PartialEq)]
enum Body {
    Function,
    Test,
    Top,
}

pub struct Checker<'a> {
    #[allow(dead_code)]
    src: &'a str,
    functions: HashMap<String, Callable>,
    structs: HashMap<String, CheckedStructure>,
    used_modules: Vec<String>,
    scopes: Vec<Scope>,
    /// Current function's declared return type (None outside functions).
    current_returns: Option<Type>,
    /// Current function declared `can fail`? (propagation rule, 13.1)
    current_can_fail: bool,
    /// Depth of enclosing `attempt` handlers (E0302 context).
    attempt_depth: usize,
    /// Loop nesting depth (stop/next legality).
    loop_depth: usize,
    /// The kind of body currently being checked (give-back rules).
    current_body: Body,
    /// Per-node types recorded while checking (keyed by node span).
    node_types: HashMap<Span, Type>,
    /// Call span → callee-name span (LOM provenance anchors, 26.5).
    callee_spans: HashMap<Span, Span>,
    /// Call spans that are flowing *reads* (`name of p`, `things at 2`) —
    /// rebuilt as ordinary calls, but classified precisely for HIR.
    read_spans: HashSet<Span>,
    pub diags: Diagnostics,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn check(program: &Program, src: &str) -> CheckedProgram {
    let mut cx = Checker {
        src,
        functions: HashMap::new(),
        structs: HashMap::new(),
        used_modules: Vec::new(),
        scopes: vec![Scope { bindings: HashMap::new(), order: Vec::new() }],
        current_returns: None,
        current_can_fail: false,
        attempt_depth: 0,
        loop_depth: 0,
        current_body: Body::Top,
        node_types: HashMap::new(),
        callee_spans: HashMap::new(),
        read_spans: HashSet::new(),
        diags: Diagnostics::new(),
    };        cx.install_builtins();
        cx.current_body = Body::Top;
    cx.check_program(program)
}

impl<'a> Checker<'a> {
    /// The M0 standard surface (S-7/S-9/S-13) and the math table (G-9).
    /// Polymorphic-shape builtins (`say`, `text`, `first`, `size`, …) are
    /// dispatched by name in `check_call`; the registered arity here is the
    /// common case used only for name resolution.
    fn install_builtins(&mut self) {
        let add = |name: &str, cx: &mut Self| {
            cx.functions.insert(
                name.to_string(),
                Callable { params: vec![], ret: Type::Error, can_fail: false, origin: Origin::Standard },
            );
        };
        for name in [
            "say", "ask", "number", "decimal", "text", "random",
            "first", "size", "join", "uppercase", "lowercase", "trim",
        ] {
            add(name, self);
        }
        for name in ["square root", "floor"] {
            self.functions.insert(
                name.to_string(),
                Callable { params: vec![], ret: Type::Error, can_fail: false, origin: Origin::Math },
            );
        }
        // `number from text` and `decimal from text` can fail (D-39); nothing
        // else in the standard surface does at M0.
        self.functions.get_mut("number").unwrap().can_fail = true;
        self.functions.get_mut("decimal").unwrap().can_fail = true;
    }

    fn is_used(&self, module: &str) -> bool {
        self.used_modules.iter().any(|m| m == module)
    }

    // -----------------------------------------------------------------------
    // Program walk (two passes: signatures first, then bodies)
    // -----------------------------------------------------------------------

    fn check_program(&mut self, program: &Program) -> CheckedProgram {
        // Pass 1: register signatures (file-relative names, call order free — 7.13).
        for item in &program.items {
            match item {
                ast::Item::Use(u) => self.register_use(u),
                ast::Item::Function(f) => self.register_function_sig(f),
                ast::Item::Structure(s) => self.register_structure(s),
                _ => {}
            }
        }
        // Pass 2: check bodies.
        let mut items = Vec::new();
        for item in &program.items {
            match item {
                ast::Item::Use(u) => items.push(CheckedItem::Use {
                    module: u.module.display(),
                    span: u.span,
                }),
                ast::Item::Function(f) => items.push(CheckedItem::Function(self.check_function(f))),
                ast::Item::Structure(s) => {
                    items.push(CheckedItem::Structure(CheckedStructure {
                        name: s.name.display(),
                        fields: s
                            .fields
                            .iter()
                            .map(|f| (f.name.display(), (&f.ty).into(), f.name.span))
                            .collect(),
                    }))
                }
                ast::Item::Test(t) => items.push(CheckedItem::Test(self.check_test(t))),
                ast::Item::Stmt(s) => {
                    self.current_body = Body::Top;
                    items.push(CheckedItem::Stmt(self.check_stmt(s)));
                }
            }
        }
        CheckedProgram {
            items,
            diags: std::mem::take(&mut self.diags),
            node_types: std::mem::take(&mut self.node_types),
            callee_spans: std::mem::take(&mut self.callee_spans),
            read_spans: std::mem::take(&mut self.read_spans),
        }
    }

    fn register_use(&mut self, u: &ast::UseDecl) {
        let module = u.module.display();
        if !matches!(module.as_str(), "math" | "random" | "standard") {
            self.diags.push(
                Diagnostic::error(
                    "E0340",
                    format!("There is no module called `{module}`."),
                    u.module.span,
                )
                .with_explanation("At M0 the modules are: `standard` (always in scope), `math`, and `random`.")
                .with_fix("use math"),
            );
        } else if !self.is_used(&module) {
            self.used_modules.push(module);
        }
    }

    fn register_function_sig(&mut self, f: &ast::FunctionDecl) {
        let name = f.name.display();
        let params: Vec<Type> = f.params.iter().map(|p| (&p.ty).into()).collect();
        let ret = f.returns.as_ref().map(|t| t.into()).unwrap_or(Type::Error);
        if self.functions.contains_key(&name) {
            self.diags.push(
                Diagnostic::error(
                    "E0341",
                    format!("There is already a function called `{name}`."),
                    f.name.span,
                ),
            );
            return;
        }
        self.functions.insert(
            name,
            Callable { params, ret, can_fail: f.can_fail, origin: Origin::User },
        );
    }

    fn register_structure(&mut self, s: &ast::StructureDecl) {
        let name = s.name.display();
        if self.structs.contains_key(&name) {
            self.diags.push(
                Diagnostic::error(
                    "E0341",
                    format!("There is already a structure called `{name}`."),
                    s.name.span,
                ),
            );
            return;
        }
        let fields: Vec<(String, Type, Span)> = s
            .fields
            .iter()
            .map(|f| (f.name.display(), (&f.ty).into(), f.name.span))
            .collect();
        self.structs.insert(
            name,
            CheckedStructure { name: s.name.display(), fields: fields.clone() },
        );
    }

    fn check_function(&mut self, f: &ast::FunctionDecl) -> CheckedFunction {
        let params: Vec<CheckedParam> = f
            .params
            .iter()
            .map(|p| CheckedParam {
                name: p.name.display(),
                ty: (&p.ty).into(),
                span: p.name.span,
            })
            .collect();
        let ret = f.returns.as_ref().map(|t| t.into());
        self.current_returns = ret.clone();
        self.current_can_fail = f.can_fail;
        self.current_body = Body::Function;
        self.push_scope();
        // §7.0.3: rejecting a parameter that prefix-shadows an existing name
        // happens at *declaration* time. Parameters live in the function scope.
        for p in &params {
            self.declare(&p.name, p.ty.clone(), false, f.span);
        }
        let body = self.check_block_stmts(&f.body);
        if let Some(rt) = &ret {
            if !matches!(rt, Type::Error) && !body_gives_back(&f.body) {
                self.diags.push(
                    Diagnostic::error(
                        "E0333",
                        format!("This function promises to give back a {} but might not.", rt.display()),
                        f.span,
                    )
                    .with_explanation("Every path through a function with a `returns` clause must end in `give back <value>`.")
                    .with_fix(format!("add `give back <a {}>` on every path", rt.display())),
                );
            }
        }
        self.pop_scope();
        self.current_returns = None;
        self.current_can_fail = false;
        CheckedFunction {
            name: f.name.display(),
            name_span: f.name.span,
            params,
            returns: ret,
            can_fail: f.can_fail,
            body,
            span: f.span,
        }
    }

    fn check_test(&mut self, t: &ast::TestDecl) -> CheckedFunction {
        self.current_returns = None;
        self.current_can_fail = false;
        self.current_body = Body::Test;
        self.push_scope();
        let body = self.check_block_stmts(&t.body);
        self.pop_scope();
        CheckedFunction {
            name: t.name.clone(),
            name_span: t.name_span,
            params: Vec::new(),
            returns: None,
            can_fail: false,
            body,
            span: t.span,
        }
    }

    // -----------------------------------------------------------------------
    // Scopes & declarations
    // -----------------------------------------------------------------------

    fn push_scope(&mut self) {
        self.scopes.push(Scope { bindings: HashMap::new(), order: Vec::new() });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Declare a binding in the current scope. §7.0.3: if the new name is a
    /// prefix of (or has as a prefix) an existing name in this scope, the
    /// declaration is rejected — greedy resolution would be ambiguous.
    fn declare(&mut self, name: &str, ty: Type, mutable: bool, span: Span) {
        let scope = self.scopes.last_mut().expect("scope stack never empty");
        // Exact redeclaration.
        if let Some(existing) = scope.bindings.get(name) {
            // §13.1: shadowing the attempt-bound names (`problem`/`result`)
            // inside their branches is allowed with a student-layer lint.
            let is_attempt_name = matches!(name, "problem" | "result");
            if is_attempt_name {
                self.diags.push(
                    Diagnostic::warning(
                        "W0332",
                        format!("`{name}` is the special name that `attempt` binds — this shadowing is allowed but can confuse readers."),
                        span,
                    )
                    .with_note(String::from("a different name would read more clearly")),
                );
                // The shadowing binding wins inside this scope.
                scope.bindings.insert(name.to_string(), Binding { ty, mutable, span });
                return;
            }
            self.diags.push(
                Diagnostic::error(
                    "E0330",
                    format!("There is already a name `{name}` in this scope."),
                    span,
                )
                .with_label(existing.span, "the first one was made here".to_string()),
            );
            return;
        }
        // Prefix ambiguity (7.0.3): `top` vs `top score`.
        let prefix_clash = scope
            .bindings
            .keys()
            .any(|k| k.starts_with(&(name.to_string() + " ")) || name.starts_with(&(k.clone() + " ")));
        if prefix_clash {
            let other = scope
                .bindings
                .keys()
                .find(|k| {
                    let spaced = format!("{k} ");
                    k.starts_with(&(name.to_string() + " ")) || name.starts_with(&spaced)
                })
                .cloned()
                .unwrap_or_default();
            self.diags.push(
                Diagnostic::error(
                    "E0331",
                    format!("The name `{name}` could be confused with `{other}` — Lagom cannot tell them apart when reading."),
                    span,
                )
                .with_explanation("Two names where one starts with the other's words (`top` and `top score`) would make greedy matching ambiguous.")
                .with_fix("rename one of them"),
            );
        }
        scope.bindings.insert(name.to_string(), Binding { ty, mutable, span });
        scope.order.push(name.to_string());
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.bindings.get(name) {
                return Some(b.clone());
            }
        }
        None
    }

    fn scope_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for scope in &self.scopes {
            for n in &scope.order {
                names.push(n.clone());
            }
        }
        for k in self.functions.keys() {
            names.push(k.clone());
        }
        names.sort();
        names.dedup();
        names
    }

    // -----------------------------------------------------------------------
    // Statements
    // -----------------------------------------------------------------------

    fn check_block_stmts(&mut self, block: &ast::Block) -> Vec<CheckedStmt> {
        self.push_scope();
        let mut out = Vec::new();
        for s in &block.stmts {
            out.push(self.check_stmt(s));
        }
        self.pop_scope();
        out
    }

    fn check_stmt(&mut self, s: &ast::Stmt) -> CheckedStmt {
        match s {
            Stmt::Make { mutable, name, value, annotation, span } => {
                let expected = annotation.as_ref().map(|t| t.into());
                let mut vt = self.check_expr(value, expected.as_ref());
                // The annotation is a contract: check the value against it and
                // the binding gets the annotated type (8.1).
                if let Some(ann) = &expected {
                    if !matches!(ann, Type::Error) {
                        self.require_assignable(ann, &vt, "the annotated value");
                        // R-15: a literal binds as the annotation says.
                        vt.ty = ann.clone();
                    }
                }
                // Shadowing the attempt-bound names gets a lint (13.1).
                if matches!(name.display().as_str(), "result" | "problem") {
                    self.diags.push(
                        Diagnostic::warning(
                            "W0332",
                            format!("`{}` is the special name that `attempt` binds — this shadowing is allowed but can confuse readers.", name.display()),
                            name.span,
                        )
                        .with_note(String::from("a different name would read more clearly")),
                    );
                }
                self.declare(&name.display(), vt.ty.clone(), *mutable, name.span);
                CheckedStmt {
                    kind: CheckedStmtKind::Make { mutable: *mutable, name: name.display(), value: vt },
                    span: *span,
                }
            }
            Stmt::Set { target, value, span } => {
                let tt = self.check_target(target, "`set`");
                let vt = self.check_expr(value, Some(&tt.ty));
                self.require_assignable(&tt.ty, &vt, "`set`");
                CheckedStmt {
                    kind: CheckedStmtKind::Set { target: tt, value: vt },
                    span: *span,
                }
            }
            Stmt::Change { decrease, target, value, span } => {
                let what = if *decrease { "`decrease`" } else { "`increase`" };
                let tt = self.check_target(target, what);
                let vt = self.check_expr(value, Some(&Type::Number));
                // increase/decrease: target and amount must be numeric (7.2).
                if !tt.ty.is_numeric() && !matches!(tt.ty, Type::Error) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0334",
                            format!("{} needs a number to change, but `{}` is {}.", what.trim_matches('`'), tt.base, tt.ty.display()),
                            tt.span,
                        )
                        .with_explanation("`increase` and `decrease` change a number by another number."),
                    );
                }
                if !vt.ty.is_numeric() && !matches!(vt.ty, Type::Error) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0334",
                            format!("{} needs a number, but the amount is {}.", what.trim_matches('`'), vt.ty.display()),
                            vt.span(),
                        ),
                    );
                }
                CheckedStmt {
                    kind: CheckedStmtKind::Change { decrease: *decrease, target: tt, value: vt },
                    span: *span,
                }
            }
            Stmt::If { branches, otherwise, span } => {
                let mut out_branches = Vec::new();
                for (cond, block) in branches {
                    let ct = self.check_expr(cond, Some(&Type::Boolean));
                    self.require_boolean(&ct, "`if`");
                    let body = self.check_block_stmts(block);
                    out_branches.push((ct, body));
                }
                let out_otherwise = otherwise.as_ref().map(|b| self.check_block_stmts(b));
                CheckedStmt {
                    kind: CheckedStmtKind::If { branches: out_branches, otherwise: out_otherwise },
                    span: *span,
                }
            }
            Stmt::Repeat(r) => self.check_repeat(r, s_stmt_span(s)),
            Stmt::Stop { span } => {
                if self.loop_depth == 0 {
                    self.diags.push(
                        Diagnostic::error("E0335", "`stop` only works inside a `repeat` loop.", *span)
                            .with_explanation("`stop` ends the turn of the loop it is inside."),
                    );
                }
                CheckedStmt { kind: CheckedStmtKind::Stop, span: *span }
            }
            Stmt::Next { span } => {
                if self.loop_depth == 0 {
                    self.diags.push(
                        Diagnostic::error("E0335", "`next` only works inside a `repeat` loop.", *span)
                            .with_explanation("`next` jumps to the loop's next turn."),
                    );
                }
                CheckedStmt { kind: CheckedStmtKind::Next, span: *span }
            }
            Stmt::GiveBack { value, span } => {
                let ret = self.current_returns.clone();
                match (&ret, self.current_body) {
                (Some(rt), Body::Function) => {
                    let vt = if matches!(rt, Type::Error) {
                        self.check_expr(value, None)
                    } else {
                        self.check_expr(value, Some(rt))
                    };
                    self.require_assignable(rt, &vt, "`give back`");
                    CheckedStmt { kind: CheckedStmtKind::GiveBack { value: vt }, span: *span }
                }
                // §7.8: `returns` is optional — inferred when absent (D-11's
                // inference-first rule applied to signatures).
                (None, Body::Function) => {
                    let vt = self.check_expr(value, None);
                    CheckedStmt { kind: CheckedStmtKind::GiveBack { value: vt }, span: *span }
                }
                _ => {
                    self.diags.push(
                        Diagnostic::error("E0336", "`give back` only works inside a function.", *span)
                            .with_explanation("Tests and top-level statements do not return values."),
                    );
                    let vt = self.check_expr(value, None);
                    CheckedStmt { kind: CheckedStmtKind::GiveBack { value: vt }, span: *span }
                }
                }
            }
            Stmt::FailWith { value, span } => {
                // §13.1: failure is a capability — the function must declare it.
                if !self.current_can_fail {
                    self.diags.push(
                        Diagnostic::error(
                            "E0337",
                            "This function does not say `can fail`, so it cannot `fail with`.",
                            *span,
                        )
                        .with_explanation("A function must declare `can fail` before it can fail.")
                        .with_fix("add a `can fail` clause to the function"),
                    );
                }
                // S-10: the failure value is a text at student level.
                let vt = self.check_expr(value, Some(&Type::Text));
                self.require_type(&vt, &Type::Text, "`fail with`", "the failure message");
                CheckedStmt { kind: CheckedStmtKind::FailWith { value: vt }, span: *span }
            }
            Stmt::Attempt { expr, tail, span } => self.check_attempt(expr, tail.as_ref(), *span),
            Stmt::CheckThat { expr, span } => {
                let vt = self.check_expr(expr, Some(&Type::Boolean));
                self.require_boolean(&vt, "`check that`");
                CheckedStmt { kind: CheckedStmtKind::CheckThat { expr: vt }, span: *span }
            }
            Stmt::ExprStmt { expr, span } => {
                let vt = self.check_expr(expr, None);
                // A bare call used for effect: a can-fail call in statement
                // position with no attempt is the unhandled-failure case (13.1).
                if let Expr::Call(call) = &vt.expr {
                    let callee = call.callee.display();
                    if let Some(sig) = self.functions.get(&callee) {
                        if sig.can_fail && self.attempt_depth == 0 {
                            self.diags.push(self.unhandled_failure(&callee, vt.span()));
                        }
                    }
                }
                CheckedStmt { kind: CheckedStmtKind::ExprStmt { expr: vt }, span: *span }
            }
        }
    }

    fn check_repeat(&mut self, r: &ast::Repeat, span: Span) -> CheckedStmt {
        match r {
            Repeat::Count { times: (v, _), binding, body, .. } => {
                self.loop_depth += 1;
                self.push_scope();
                if let Some(b) = binding {
                    self.declare(&b.display(), Type::Number, false, b.span);
                }
                let out_body = self.check_stmt_list(body);
                self.pop_scope();
                self.loop_depth -= 1;
                CheckedStmt {
                    kind: CheckedStmtKind::RepeatCount {
                        times: *v,
                        binding: binding.as_ref().map(|b| b.display()),
                        body: out_body,
                    },
                    span,
                }
            }
            Repeat::While { cond, body, .. } => {
                let ct = self.check_expr(cond, Some(&Type::Boolean));
                self.require_boolean(&ct, "`repeat while`");
                self.loop_depth += 1;
                self.push_scope();
                let out_body = self.check_stmt_list(body);
                self.pop_scope();
                self.loop_depth -= 1;
                CheckedStmt {
                    kind: CheckedStmtKind::RepeatWhile { cond: ct, body: out_body },
                    span,
                }
            }
            Repeat::ForEach { item, index, iter, body, .. } => {
                let it = self.check_expr(iter, None);
                let elem = match &it.ty {
                    Type::List(e) => (**e).clone(),
                    Type::Map(k, v) => Type::Pair(Box::new((**k).clone()), Box::new((**v).clone())),
                    Type::Error => Type::Error,
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                "E0338",
                                format!("`for each` needs a list or a map, but this is {}.", other.display()),
                                it.span(),
                            )
                            .with_explanation("`repeat for each item in <list>` walks through the list's items."),
                        );
                        Type::Error
                    }
                };
                self.loop_depth += 1;
                self.push_scope();
                self.declare(&item.display(), elem, false, item.span);
                if let Some(ix) = index {
                    self.declare(&ix.display(), Type::Number, false, ix.span);
                }
                let out_body = self.check_stmt_list(body);
                self.pop_scope();
                self.loop_depth -= 1;
                CheckedStmt {
                    kind: CheckedStmtKind::RepeatForEach {
                        item: item.display(),
                        index: index.as_ref().map(|i| i.display()),
                        iter: it,
                        body: out_body,
                    },
                    span,
                }
            }
        }
    }

    fn check_stmt_list(&mut self, block: &ast::Block) -> Vec<CheckedStmt> {
        block.stmts.iter().map(|s| self.check_stmt(s)).collect()
    }

    /// `attempt <expr> [tail]` (13.1, G-10): tails bind `problem` (failure
    /// branch, text) and `result` (success branch, the attempt's type); `as`
    /// binds the error under a chosen name. Bare attempts propagate (E0332).
    fn check_attempt(
        &mut self,
        expr: &Expr,
        tail: Option<&ast::AttemptTail>,
        span: Span,
    ) -> CheckedStmt {
        // A bare attempt hands failure to *this* function's caller.
        if tail.is_none() && !self.current_can_fail {
            self.diags.push(
                Diagnostic::error(
                    "E0332",
                    "An `attempt` without a failure branch passes the problem on — this function must say `can fail`.",
                    span,
                )
                .with_explanation("A bare attempt hands any failure to this function's caller, which is only possible if the function declares `can fail`.")
                .with_fix("add a `can fail` clause to this function, or handle it with `if it fails then … otherwise …`"),
            );
        }
        let saved_fail = self.current_can_fail;
        // Inside the attempt's success path, failures are handled here, so the
        // propagation requirement no longer applies to nested bare attempts.
        if tail.is_some() {
            self.current_can_fail = false;
        }
        self.attempt_depth += 1;
        let inner = self.check_expr(expr, None);
        self.attempt_depth -= 1;
        self.current_can_fail = saved_fail;

        let mut out_tail = None;
        if let Some(tail) = tail {
            match tail {
                ast::AttemptTail::Propagate => {
                    // The readable `?`: requires the enclosing function to fail.
                    if !self.current_can_fail {
                        self.diags.push(
                            Diagnostic::error(
                                "E0332",
                                "`and pass the problem on` needs the function itself to say `can fail`.",
                                span,
                            )
                            .with_explanation("Passing a problem on hands it to this function's caller — so this function must be able to fail too.")
                            .with_fix("add a `can fail` clause to this function"),
                        );
                    }
                    out_tail = Some(CheckedAttemptTail::Propagate);
                }
                ast::AttemptTail::IfItFails { then_block, otherwise } => {
                    self.push_scope();
                    self.declare("problem", Type::Text, false, span);
                    let then_body = self.check_stmt_list_scoped(then_block);
                    self.pop_scope();
                    let otherwise_body = otherwise.as_ref().map(|ob| {
                        self.push_scope();
                        // G-10: the success value is bound to the fixed name
                        // `result` (13.1's own mechanism).
                        self.declare("result", inner.ty.clone(), false, inner.span());
                        let b = self.check_stmt_list_scoped(ob);
                        self.pop_scope();
                        b
                    });
                    out_tail = Some(CheckedAttemptTail::IfItFails {
                        then_block: then_body,
                        otherwise: otherwise_body,
                    });
                }
                ast::AttemptTail::As { name, block, otherwise } => {
                    self.push_scope();
                    self.declare(&name.display(), Type::Text, false, name.span);
                    let as_body = self.check_stmt_list_scoped(block);
                    self.pop_scope();
                    let otherwise_body = otherwise.as_ref().map(|ob| {
                        self.push_scope();
                        self.declare("result", inner.ty.clone(), false, inner.span());
                        let b = self.check_stmt_list_scoped(ob);
                        self.pop_scope();
                        b
                    });
                    out_tail = Some(CheckedAttemptTail::As {
                        name: name.display(),
                        name_span: name.span,
                        block: as_body,
                        otherwise: otherwise_body,
                    });
                }
            }
        }
        CheckedStmt {
            kind: CheckedStmtKind::Attempt { inner, tail: out_tail },
            span,
        }
    }

    fn check_stmt_list_scoped(&mut self, block: &ast::Block) -> Vec<CheckedStmt> {
        block.stmts.iter().map(|s| self.check_stmt(s)).collect()
    }

    /// Check a `set`/`increase`/`decrease` target: base binding exists, is
    /// mutable, and each accessor type-checks (7.2/7.6). G-13: a leading
    /// `first of L` read is accepted as the target-equivalent of `L at 0`.
    fn check_target(&mut self, target: &ast::Target, what: &str) -> TypedTarget {
        let base_name = target.base.display();
        // G-13: `first of <list>` in target position. The grammar's
        // `target = name { prep additive }` cannot derive a call head, so sema
        // rewrites `set first of ps to …` into `set ps at 0 to …` (the two
        // access forms are equivalent by 7.6).
        if base_name == "first"
            && target.accessors.len() >= 1
            && matches!(target.accessors[0], Accessor::Of { .. })
            && self.lookup("first").is_none()
            && self.functions.contains_key("first")
        {
            let base_of = match &target.accessors[0] {
                Accessor::Of { field, span } => {
                    let _ = field;
                    *span
                }
                _ => unreachable!(),
            };
            let mut rewritten = ast::Target {
                base: Name { words: vec!["ps".into()], span: base_of },
                accessors: Vec::new(),
                span: target.span,
            };
            // The real base name comes from the `of` argument, parsed here as
            // a single-word name (the M0 grammar's `prep additive` accepts
            // exactly an additive — a plain name argument).
            if let Accessor::Of { field, .. } = &target.accessors[0] {
                let _ = field;
            }
            // Extract the argument expression: `prep additive` stores it in
            // the accessor's expression — but Target::Of carries only a name.
            // The parser's target production binds `of <name>` here, so the
            // base is the field name itself.
            if let Accessor::Of { field, .. } = &target.accessors[0] {
                rewritten.base = field.clone();
            }
            rewritten.accessors.push(Accessor::At {
                index: Expr::Int { value: 0, span: base_of },
                span: base_of,
            });
            for acc in target.accessors.iter().skip(1) {
                rewritten.accessors.push(acc.clone());
            }
            return self.check_target(&rewritten, what);
        }
        // Flowing field targets (7.11's symmetry with reads): `set score of
        // p to 10` parses with the *field* first — mirror of the read `name
        // of p`. When the base is not a known binding but the `of` argument
        // is, and the argument's type carries the field, rewrite to the
        // ordinary form `set p's-score` (base p, accessor `of score`).
        //
        // The parser stores `Accessor::Of { field }` with `field: Name`; for
        // `score of p` that name is `p` and the base name is `score`.
        if !self.lookup(&base_name).is_some()
            && target.accessors.len() == 1
        {
            if let Accessor::Of { field, span } = &target.accessors[0] {
                let arg_name = field.display();
                if self.lookup(&arg_name).is_some() {
                    let arg_ty = self.lookup(&arg_name).map(|b| b.ty).unwrap_or(Type::Error);
                    if let Type::Struct(_) = arg_ty {
                        let rewritten = ast::Target {
                            base: field.clone(),
                            accessors: vec![Accessor::Of {
                                field: target.base.clone(),
                                span: *span,
                            }],
                            span: target.span,
                        };
                        return self.check_target(&rewritten, what);
                    }
                }
            }
        }
        let base_binding = match self.lookup(&base_name) {
            Some(b) => Some(b),
            None => {
                self.diags.push(self.unknown_name(&base_name, target.base.span));
                None
            }
        };
        if let Some(b) = &base_binding {
            if !b.mutable {
                self.diags.push(
                    Diagnostic::error(
                        "E0339",
                        format!("`{base_name}` was made without `changing`, so it cannot be changed."),
                        target.base.span,
                    )
                    .with_label(b.span, String::from("made here (immutable)"))
                    .with_fix(format!("make changing {base_name} equal to …")),
                );
            }
        }
        let mut out = TypedTarget {
            base: base_name,
            accessors: Vec::new(),
            ty: base_binding.map(|b| b.ty).unwrap_or(Type::Error),
            span: target.span,
        };
        for acc in &target.accessors {
            match acc {
                Accessor::At { index, span } => {
                    // A list index is a number; a map index is the map's key
                    // type (7.6). When the base is not yet known, expect a
                    // number — the common case.
                    let want = match &out.ty {
                        Type::Map(k, _) => (**k).clone(),
                        _ => Type::Number,
                    };
                    let it = self.check_expr(index, Some(&want));
                    self.require_type(&it, &want, "a list or map index", "the index");
                    out.ty = match out.ty.clone() {
                        Type::List(e) => (*e).clone(),
                        Type::Map(_, v) => (*v).clone(),
                        Type::Error => Type::Error,
                        other => {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0342",
                                    format!("`at` needs a list or a map, but this is {}.", other.display()),
                                    *span,
                                ),
                            );
                            Type::Error
                        }
                    };
                    out.accessors.push(TypedAccessor::At { index: it });
                }
                Accessor::Of { field, span } => {
                    out.ty = match out.ty.clone() {
                        Type::Struct(sname) => {
                            let field_name = field.display();
                            match self.struct_field(&sname, &field_name) {
                                Some(t) => t,
                                None => {
                                    let fields = self.structs.get(&sname)
                                        .map(|s| s.fields.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>().join(", "))
                                        .unwrap_or_default();
                                    self.diags.push(
                                        Diagnostic::error(
                                            "E0343",
                                            format!("`{sname}` has no field called `{field_name}`."),
                                            *span,
                                        )
                                        .with_note(format!("its fields are: {fields}")),
                                    );
                                    Type::Error
                                }
                            }
                        }
                        Type::Error => Type::Error,
                        other => {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0343",
                                    format!("`of` reads a structure's field, but `{}` is {}.", field.display(), other.display()),
                                    *span,
                                ),
                            );
                            Type::Error
                        }
                    };
                    out.accessors.push(TypedAccessor::Of { field: field.display() });
                }
            }
        }
        let _ = what;
        out
    }

    fn struct_field(&self, sname: &str, field: &str) -> Option<Type> {
        self.structs
            .get(sname)
            .and_then(|s| s.fields.iter().find(|(n, _, _)| n == field).map(|(_, t, _)| t.clone()))
    }

    fn unknown_name(&self, name: &str, span: Span) -> Diagnostic {
        let mut d = Diagnostic::error(
            "E0344",
            format!("I do not know what `{name}` is. Make it first with `make`."),
            span,
        )
        .with_explanation("Names must be made with `make` (or come from `takes`, `using`, or a loop) before they are used.");
        let suggestions = did_you_mean(name, &self.scope_names());
        if let Some(best) = suggestions.first() {
            d = d.with_fix(format!("did you mean `{best}`?"));
        }
        d
    }

    // -----------------------------------------------------------------------
    // Expressions
    // -----------------------------------------------------------------------

    /// Check an expression. `expected` is the inference hint (R-15: numeric
    /// literals unify with it when compatible; D-11: otherwise number).
    fn check_expr(&mut self, e: &Expr, expected: Option<&Type>) -> TypedExpr {
        // Record the type of every node (root and children) keyed by span —
        // the side table HIR reads so desugared nodes keep static types.
        let out = self.check_expr_inner(e, expected);
        self.node_types.insert(expr_span(&out.expr), out.ty.clone());
        out
    }

    /// Check an expression; `check_expr` wraps this to record the type table.
    fn check_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> TypedExpr {
        match e {
            Expr::Int { .. } => {
                // R-15: literal unifies with the expected numeric type; the
                // un-constrained default is `number` (D-10).
                let ty = match expected {
                    Some(Type::Decimal) => Type::Decimal,
                    Some(Type::NumericLit) => Type::NumericLit,
                    _ => Type::Number,
                };
                TypedExpr { expr: e.clone(), ty }
            }
            Expr::Float { .. } => TypedExpr { expr: e.clone(), ty: Type::Decimal },
            Expr::Text { .. } => TypedExpr { expr: e.clone(), ty: Type::Text },
            Expr::Bool { .. } => TypedExpr { expr: e.clone(), ty: Type::Boolean },
            Expr::Nothing { .. } => TypedExpr { expr: e.clone(), ty: Type::NothingLit },
            Expr::Name { name, span } => self.check_name_expr(name, *span),
            Expr::Interp { parts, span } => {
                // S-9: every M0 value prints; check each embedded expression and
                // keep the checked form — dropping it here left nested calls
                // (`say "{greet who}"`) untyped in HIR, silently `nothing`.
                let mut out = Vec::new();
                for part in parts {
                    match part {
                        ast::InterpPart::Lit(s) => out.push(ast::InterpPart::Lit(s.clone())),
                        ast::InterpPart::Expr(inner) => {
                            let t = self.check_expr(inner, None);
                            out.push(ast::InterpPart::Expr(t.expr));
                        }
                    }
                }
                TypedExpr { expr: Expr::Interp { parts: out, span: *span }, ty: Type::Text }
            }
            Expr::Group { inner, .. } => self.check_expr(inner, expected),
            Expr::Neg { inner, span } => {
                let it = self.check_expr(inner, expected);
                if !it.ty.is_numeric() && !matches!(it.ty, Type::Error) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0345",
                            format!("You can only negate a number, but this is {}.", it.ty.display()),
                            *span,
                        ),
                    );
                }
                TypedExpr { expr: e.clone(), ty: it.ty }
            }
            Expr::Not { inner, span: _ } => {
                let it = self.check_expr(inner, Some(&Type::Boolean));
                self.require_boolean(&it, "`not`");
                TypedExpr { expr: e.clone(), ty: Type::Boolean }
            }
            Expr::Binary { op, left, right, span } => {
                self.check_binary(*op, left, right, *span, expected)
            }
            Expr::ListLit { elements, span: _ } => {
                let elem_expected = match expected {
                    Some(Type::List(e)) => Some((**e).clone()),
                    _ => None,
                };
                let mut elem_ty: Option<Type> = None;
                for el in elements {
                    let et = self.check_expr(el, elem_expected.as_ref());
                    match &et.ty {
                        Type::Error => {}
                        t => match &elem_ty {
                            None => elem_ty = Some(t.clone()),
                            Some(prev) => match unify_numeric(prev, t) {
                                Some(u) => elem_ty = Some(u),
                                None => {
                                    self.diags.push(
                                        Diagnostic::error(
                                            "E0346",
                                            format!("This list mixes {} and {}.", prev.display(), t.display()),
                                            expr_span(el),
                                        )
                                        .with_explanation("Every item in a list must have the same type."),
                                    );
                                }
                            },
                        },
                    }
                }
                let elem = elem_ty.or(elem_expected).unwrap_or(Type::Error);
                TypedExpr { expr: e.clone(), ty: Type::List(Box::new(elem)) }
            }
            Expr::MapLit { entries, .. } => {
                let mut k_ty: Option<Type> = None;
                let mut v_ty: Option<Type> = None;
                for (k, v) in entries {
                    let kt = self.check_expr(k, None);
                    let vt = self.check_expr(v, None);
                    k_ty = Some(unify_opt(k_ty, kt.ty));
                    v_ty = Some(unify_opt(v_ty, vt.ty));
                }
                let k = k_ty.unwrap_or(Type::Error);
                let v = v_ty.unwrap_or(Type::Error);
                TypedExpr {
                    expr: e.clone(),
                    ty: Type::Map(Box::new(k), Box::new(v)),
                }
            }
            Expr::PairLit { first, second, .. } => {
                let ft = self.check_expr(first, None);
                let st = self.check_expr(second, None);
                TypedExpr {
                    expr: e.clone(),
                    ty: Type::Pair(Box::new(ft.ty), Box::new(st.ty)),
                }
            }
            Expr::StructLit { name, fields, span } => self.check_struct_lit(name, fields, *span, e),
            Expr::AttemptExpr { expr, span } => {
                // The bare attempt as an expression (13.1): evaluates to the
                // success value; the enclosing function must be able to fail.
                if !self.current_can_fail {
                    self.diags.push(
                        Diagnostic::error(
                            "E0332",
                            "An `attempt` without a failure branch passes the problem on — this function must say `can fail`.",
                            *span,
                        )
                        .with_explanation("A bare attempt hands any failure to this function's caller, which is only possible if the function declares `can fail`.")
                        .with_fix("add a `can fail` clause to this function, or handle it with `if it fails then … otherwise …`"),
                    );
                }
                self.attempt_depth += 1;
                let inner = self.check_expr(expr, expected);
                self.attempt_depth -= 1;
                TypedExpr { expr: e.clone(), ty: inner.ty }
            }
            Expr::Call(call) => self.check_call(call, expected),
        }
    }

    fn check_struct_lit(
        &mut self,
        name: &Name,
        fields: &[(Name, Expr)],
        span: Span,
        outer: &Expr,
    ) -> TypedExpr {
        let sname = name.display();
        let def = self.structs.get(&sname).cloned();
        match def {
            None => {
                self.diags.push(
                    Diagnostic::error(
                        "E0347",
                        format!("There is no structure called `{sname}`."),
                        name.span,
                    ),
                );
                TypedExpr { expr: Expr::Name { name: name.clone(), span }, ty: Type::Error }
            }
            Some(def) => {
                let mut given: Vec<(String, Type)> = Vec::new();
                let mut out_fields: Vec<(Name, Expr)> = Vec::new();
                for (fname, fexpr) in fields {
                    let ft = self.check_expr(fexpr, None);
                    let fname_s = fname.display();
                    match def.fields.iter().find(|(n, _, _)| n == &fname_s) {
                        Some((_, fty, _)) => {
                            let ft2 = TypedExpr { expr: fexpr.clone(), ty: ft.ty.clone() };
                            self.require_assignable(fty, &ft2, &format!("the {sname}'s `{fname_s}` field"));
                            out_fields.push((fname.clone(), ft2.expr));
                        }
                        None => {
                            let fields_list = def.fields.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>().join(", ");
                            self.diags.push(
                                Diagnostic::error(
                                    "E0348",
                                    format!("`{sname}` has no field called `{fname_s}`."),
                                    fname.span,
                                )
                                .with_note(format!("its fields are: {fields_list}")),
                            );
                        }
                    }
                    given.push((fname_s, ft.ty));
                }
                for (n, _, _) in &def.fields {
                    let count = given.iter().filter(|(g, _)| g == n).count();
                    if count == 0 {
                        self.diags.push(
                            Diagnostic::error(
                                "E0349",
                                format!("This {sname} is missing its `{n}` field."),
                                span,
                            )
                            .with_fix(format!("a {sname} with {n} …")),
                        );
                    } else if count > 1 {
                        self.diags.push(
                            Diagnostic::error(
                                "E0350",
                                format!("This {sname} gives `{n}` twice."),
                                span,
                            ),
                        );
                    }
                }
                TypedExpr { expr: outer.clone(), ty: Type::Struct(sname) }
            }
        }
    }

    /// §7.0.3 greedy resolution of a word-run in expression position.
    fn check_name_expr(&mut self, name: &Name, span: Span) -> TypedExpr {
        let full = name.display();
        // 1. Whole-run binding (the common case: single word).
        if let Some(b) = self.lookup(&full) {
            self.node_types.insert(span, b.ty.clone());
            return TypedExpr { expr: Expr::Name { name: name.clone(), span }, ty: b.ty };
        }
        // 1.5. Whole-run function with zero arguments (G-15, docs/14): the
        //      parser emits a bare multi-word name run when a call has no
        //      argument triggers, so `risky business` (a can-fail function
        //      taking nothing) arrives here as a `Name`. The whole run wins
        //      before any split — the longest known reading (7.0.3).
        if self.functions.contains_key(&full) {
            let call = CallExpr {
                callee: Name { words: vec![full.clone()], span },
                first: None,
                preps: Vec::new(),
                and_args: Vec::new(),
                with_args: Vec::new(),
                span,
            };
            let rebuilt = Expr::Call(Box::new(call));
            let ty = match &rebuilt {
                Expr::Call(c) => self.check_call(c, None).ty,
                _ => unreachable!(),
            };
            return TypedExpr { expr: rebuilt, ty };
        }
        // 2. Greedy split (7.0.3): a known-callable prefix with the rest as
        //    its first argument — `greet name` when `greet` is a function and
        //    `name` a binding. Longest prefix wins.
        for split in (1..name.words.len()).rev() {
            let head = name.words[..split].join(" ");
            if self.functions.contains_key(&head) {
                // The tail words occupy the *end* of the run in the source;
                // giving the synthetic argument its true sub-span keeps the
                // node-type table collision-free (the call node and its arg
                // must never share a span — HIR keys types on spans).
                let tail_span = self.tail_words_span(span, &name.words, split);
                let arg = Expr::Name {
                    name: Name { words: name.words[split..].to_vec(), span: tail_span },
                    span: tail_span,
                };
                let call = CallExpr {
                    callee: Name { words: vec![head], span },
                    first: Some(Box::new(ast::Arg { expr: Box::new(arg), span: tail_span })),
                    preps: Vec::new(),
                    and_args: Vec::new(),
                    with_args: Vec::new(),
                    span,
                };
                // Check through check_call so the argument is itself checked
                // (nested splits resolve) and the rebuilt call carries the
                // checked form — returning the pre-check clone here dropped
                // the argument's own checking and left HIR with an untyped
                // multi-word name that silently evaluated to nothing.
                return self.check_call(&call, None);
            }
        }
        // 3. Unknown.
        let d = self.unknown_name(&full, span);
        self.diags.push(d);
        TypedExpr { expr: Expr::Name { name: name.clone(), span }, ty: Type::Error }
    }

    fn check_call(&mut self, call: &ast::CallExpr, expected: Option<&Type>) -> TypedExpr {
        // §7.0.3 greedy callee resolution: a multi-word callee whose known-
        // function prefix splits leaves the rest as the argument call
        // (`say square root of 16` parses as callee `say square root` →
        // say(square root(16))).
        let resolved = self.resolve_callee(call);
        self.check_resolved_call(&resolved, expected)
    }

    /// The sub-span of `span` covering `words[split..]`. The words were lexed
    /// as one space-separated run, so their extent is computable from the
    /// source; falls back to the whole span if the text does not match
    /// (defensive — the lexer guarantees the joined form).
    fn tail_words_span(&self, span: Span, words: &[String], split: usize) -> Span {
        let Some(text) = self.src.get(span.start..span.end) else {
            return span;
        };
        let bytes = text.as_bytes();
        let mut i = 0usize;
        for word in &words[..split] {
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            if text[i..].starts_with(word.as_str()) {
                i += word.len();
            } else {
                return span;
            }
        }
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        Span { start: span.start + i, end: span.end }
    }

    /// Rewrite a call whose multi-word callee contains a known-function
    /// prefix: the longest such prefix becomes the callee and the remaining
    /// words plus the original arguments become its single argument call.
    fn resolve_callee(&self, call: &ast::CallExpr) -> ast::CallExpr {
        let words = &call.callee.words;
        if words.len() > 1 {
            for split in (1..words.len()).rev() {
                let head = words[..split].join(" ");
                if self.functions.contains_key(&head) {
                    // Same invariant as the name-split path: the synthetic
                    // argument (and the inner call it wraps) takes the tail
                    // words' own span, distinct from the outer call's.
                    let tail_span = self.tail_words_span(call.callee.span, words, split);
                    let inner = ast::CallExpr {
                        callee: Name { words: words[split..].to_vec(), span: tail_span },
                        first: call.first.clone(),
                        preps: call.preps.clone(),
                        and_args: call.and_args.clone(),
                        with_args: call.with_args.clone(),
                        span: tail_span,
                    };
                    return ast::CallExpr {
                        callee: Name { words: vec![head], span: call.callee.span },
                        first: Some(Box::new(ast::Arg {
                            expr: Box::new(Expr::Call(Box::new(inner))),
                            span: tail_span,
                        })),
                        preps: Vec::new(),
                        and_args: Vec::new(),
                        with_args: Vec::new(),
                        span: call.span,
                    };
                }
            }
        }
        call.clone()
    }

    fn check_resolved_call(&mut self, call: &ast::CallExpr, _expected: Option<&Type>) -> TypedExpr {
        let callee = call.callee.display();
        // LOM provenance: function-call events key on the callee span (26.5).
        self.callee_spans.insert(call.span, call.callee.span);
        // Arguments in call order.
        let mut args: Vec<&Expr> = Vec::new();
        if let Some(first) = &call.first {
            args.push(&first.expr);
        }
        for (_, a) in &call.preps {
            args.push(&a.expr);
        }
        for a in &call.and_args {
            args.push(&a.expr);
        }
        for (_, a) in &call.with_args {
            args.push(&a.expr);
        }
        let sig = self.functions.get(&callee).cloned();
        match sig {
            None => {
                // Flowing field/index reads (7.6/7.11): `name of p`,
                // `things at 2` — a non-function callee with exactly one
                // prepositional argument is a read, not a call. For `of`, the
                // argument is the base and the callee names the field; for
                // `at`, the callee is the base and the argument the index.
                if call.first.is_none()
                    && call.and_args.is_empty()
                    && call.with_args.is_empty()
                    && call.preps.len() == 1
                {
                    let (prep, arg_expr, arg_span) = {
                        let (p, a) = &call.preps[0];
                        (*p, &a.expr, a.span)
                    };
                    match prep {
                        Prep::Of => {
                            let base = self.check_expr(arg_expr, None);
                            let rebuilt = self.rebuild_call(call, vec![base.clone()]);
                            self.read_spans.insert(call.span);
                            return self.field_read(&callee, base, rebuilt, arg_span);
                        }
                        Prep::At => {
                            let base = self.check_name_expr(&call.callee, call.callee.span);
                            let idx_expected = match &base.ty {
                                Type::List(_) => Some(Type::Number),
                                Type::Map(k, _) => Some((**k).clone()),
                                _ => None,
                            };
                            let index = self.check_expr(arg_expr, idx_expected.as_ref());
                            let rebuilt = self.rebuild_call(call, vec![index.clone()]);
                            self.read_spans.insert(call.span);
                            return self.index_read(base, index, rebuilt, arg_span);
                        }
                        _ => {}
                    }
                }
                if self.lookup(&callee).is_some() {
                    self.diags.push(
                        Diagnostic::error(
                            "E0351",
                            format!("`{callee}` is a value, not a function — it cannot take arguments."),
                            call.callee.span,
                        ),
                    );
                } else {
                    let d = self.unknown_name(&callee, call.callee.span);
                    self.diags.push(d);
                }
                for a in &args {
                    self.check_expr(a, None);
                }
                TypedExpr { expr: self.rebuild_call(call, Vec::new()), ty: Type::Error }
            }
            Some(sig) => {
                let checked: Vec<TypedExpr> =
                    args.iter().map(|a| self.check_expr(a, None)).collect();
                let n = checked.len();
                let (ty, can_fail) = match callee.as_str() {
                    // ----- output/input (7.1, S-9) -----
                    "say" => {
                        // One argument of any type (everything prints, S-9).
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        }
                        (Type::Text, false)
                    }
                    "ask" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`ask`", "the question");
                        }
                        (Type::Text, false)
                    }
                    // ----- conversions (D-39, S-7) — `number from text` can fail -----
                    "number" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`number from`", "the text to convert");
                        }
                        (Type::Number, true)
                    }
                    "decimal" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`decimal from`", "the text to convert");
                        }
                        (Type::Decimal, true)
                    }
                    "text" => {
                        // `text from <value>`: any M0 value formats (S-9).
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        }
                        (Type::Text, false)
                    }
                    "random" => {
                        // `random from 1 to 6`: two numbers (S-7).
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        }
                        for c in &checked {
                            self.require_type(c, &Type::Number, "`random from … to …`", "a number");
                        }
                        (Type::Number, false)
                    }
                    // ----- collection queries (7.6, S-13) -----
                    "first" => {
                        // `first of <list>` → the element as an option (S-13).
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                            (Type::Error, false)
                        } else {
                            match &checked[0].ty {
                                Type::List(e) => (Type::Option(e.clone()), false),
                                Type::Error => (Type::Error, false),
                                other => {
                                    self.diags.push(
                                        Diagnostic::error(
                                            "E0352",
                                            format!("`first of` needs a list, but this is {}.", other.display()),
                                            checked[0].span(),
                                        ),
                                    );
                                    (Type::Error, false)
                                }
                            }
                        }
                    }
                    "size" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            match &checked[0].ty {
                                Type::List(_) | Type::Map(_, _) | Type::Text | Type::Error => {}
                                other => {
                                    self.diags.push(
                                        Diagnostic::error(
                                            "E0352",
                                            format!("`size of` needs a list, map, or text, but this is {}.", other.display()),
                                            checked[0].span(),
                                        ),
                                    );
                                }
                            }
                        }
                        (Type::Number, false)
                    }
                    "join" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::List(Box::new(Type::Text)), "`join`", "a list of text");
                        }
                        (Type::Text, false)
                    }
                    // ----- text operations -----
                    "uppercase" | "lowercase" | "trim" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, &format!("`{callee}`"), "the text");
                        }
                        (Type::Text, false)
                    }
                    // ----- math module (G-9) -----
                    "square root" | "floor" => {
                        if !self.is_used("math") {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0353",
                                    format!("`{callee}` is in the `math` module — write `use math` at the top of the file."),
                                    call.callee.span,
                                )
                                .with_fix("use math"),
                            );
                        }
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Number, &format!("`{callee}`"), "a number");
                        }
                        let ty = if callee == "square root" { Type::Decimal } else { Type::Number };
                        (ty, false)
                    }
                    // ----- user functions -----
                    _ => {
                        if n != sig.params.len() {
                            self.diags.push(self.bad_arity(&callee, sig.params.len(), n, call.callee.span));
                        }
                        for (i, c) in checked.iter().enumerate() {
                            if let Some(pt) = sig.params.get(i) {
                                if !matches!(pt, Type::Error) && !matches!(c.ty, Type::Error) {
                                    self.require_assignable(pt, c, "an argument");
                                }
                            }
                        }
                        // Unhandled can-fail call in expression position (13.1):
                        // legal only inside an attempt's checked region.
                        if sig.can_fail && self.attempt_depth == 0 {
                            self.diags.push(self.unhandled_failure(&callee, call.span));
                        }
                        let ret = if matches!(sig.ret, Type::Error) {
                            Type::Error
                        } else {
                            sig.ret.clone()
                        };
                        (ret, sig.can_fail)
                    }
                };
                // The E0302 rule keys off can-fail *sites*, not signatures: for
                // builtins the can-fail-ness comes from the table above.
                if can_fail && self.attempt_depth == 0 && !matches!(sig.origin, Origin::User) {
                    self.diags.push(self.unhandled_failure(&callee, call.span));
                }
                let rebuilt = self.rebuild_call(call, checked);
                TypedExpr { expr: rebuilt, ty }
            }
        }
    }

    /// Rebuild the call expression with checked argument expressions — the
    /// resolved form downstream stages consume.
    fn rebuild_call(&self, call: &ast::CallExpr, checked: Vec<TypedExpr>) -> Expr {
        let mut it = checked.into_iter();
        let mut out = call.clone();
        if let Some(first) = out.first.as_mut() {
            if let Some(t) = it.next() {
                first.expr = Box::new(t.expr);
            }
        }
        for (_, a) in out.preps.iter_mut() {
            if let Some(t) = it.next() {
                a.expr = Box::new(t.expr);
            }
        }
        for a in out.and_args.iter_mut() {
            if let Some(t) = it.next() {
                a.expr = Box::new(t.expr);
            }
        }
        for (_, a) in out.with_args.iter_mut() {
            if let Some(t) = it.next() {
                a.expr = Box::new(t.expr);
            }
        }
        Expr::Call(Box::new(out))
    }

    /// A flowing field read (`name of p`): the callee names the field, the
    /// `of`-argument is the base. Type comes from the structure's field table.
    fn field_read(&mut self, field: &str, base: TypedExpr, rebuilt: Expr, span: Span) -> TypedExpr {
        let ty = match &base.ty {
            Type::Struct(sname) => match self.struct_field(sname, field) {
                Some(t) => t,
                None => {
                    let fields = self
                        .structs
                        .get(sname)
                        .map(|s| s.fields.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>().join(", "))
                        .unwrap_or_default();
                    self.diags.push(
                        Diagnostic::error(
                            "E0343",
                            format!("`{sname}` has no field called `{field}."),
                            span,
                        )
                        .with_note(format!("its fields are: {fields}")),
                    );
                    Type::Error
                }
            },
            Type::Error => Type::Error,
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0343",
                        format!("`of` reads a structure's field, but `{field}` was used on {}, which is not a structure.", other.display()),
                        span,
                    ),
                );
                Type::Error
            }
        };
        TypedExpr { expr: rebuilt, ty }
    }

    /// A flowing index read (`things at 2`): the callee is the base, the `at`
    /// argument is the index.
    fn index_read(&mut self, base: TypedExpr, index: TypedExpr, rebuilt: Expr, span: Span) -> TypedExpr {
        let ty = match &base.ty {
            Type::List(e) => {
                if !matches!(index.ty, Type::Number | Type::Error) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0342",
                            format!("A list index must be a number, but this is {}.", index.ty.display()),
                            span,
                        ),
                    );
                }
                (**e).clone()
            }
            Type::Map(_, v) => {
                if !matches!(index.ty, Type::Error) && !same_key_type(&base.ty, &index.ty) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0342",
                            format!("This map is keyed by {} but the key is {}.", map_key_display(&base.ty), index.ty.display()),
                            span,
                        ),
                    );
                }
                (**v).clone()
            }
            Type::Error => Type::Error,
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0342",
                        format!("`at` needs a list or a map, but this is {}.", other.display()),
                        span,
                    ),
                );
                Type::Error
            }
        };
        TypedExpr { expr: rebuilt, ty }
    }

    fn unhandled_failure(&self, callee: &str, span: Span) -> Diagnostic {
        Diagnostic::error(
            "E0302",
            format!("`{callee}` can fail, so this call must be wrapped in `attempt`."),
            span,
        )
        .with_explanation("A function that can fail must be handled: wrap the call in `attempt … if it fails then … otherwise …`, bind the problem with `as`, or pass it on with `and pass the problem on`.")
        .with_fix(format!("attempt {callee} … if it fails then\n    say problem\notherwise\n    say result"))
    }

    fn bad_arity(&self, callee: &str, want: usize, got: usize, span: Span) -> Diagnostic {
        Diagnostic::error(
            "E0354",
            format!("`{callee}` needs {want} value{}, but this call gives {got}.", if want == 1 { "" } else { "s" }),
            span,
        )
    }

    // -----------------------------------------------------------------------
    // Binary operators (D-30, S-1/S-3/S-4)
    // -----------------------------------------------------------------------

    fn check_binary(
        &mut self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
        span: Span,
        expected: Option<&Type>,
    ) -> TypedExpr {
        // Equality: structural, same-typed values (D-30). `nothing` compares
        // only against options (8.5: no null exists to compare with).
        if matches!(op, BinOp::Equal | BinOp::NotEqual) {
            let lt = self.check_expr(left, None);
            let rt = self.check_expr(right, None);
            let ok = match (&lt.ty, &rt.ty) {
                (Type::Error, _) | (_, Type::Error) => true,
                (Type::NothingLit, other) | (other, Type::NothingLit) => {
                    matches!(other, Type::Option(_) | Type::NothingLit)
                }
                (a, b) => unify_numeric(a, b).is_some() || a == b,
            };
            if !ok {
                self.diags.push(
                    Diagnostic::error(
                        "E0355",
                        format!("You cannot compare {} with {}.", lt.ty.display(), rt.ty.display()),
                        span,
                    )
                    .with_explanation("Two values can be compared only when they have the same type."),
                );
            }
            return TypedExpr { expr: e_binary(op, left, right, span), ty: Type::Boolean };
        }
        // Ordering: numbers only (no truthiness — 8.3).
        if matches!(op, BinOp::Greater | BinOp::Less | BinOp::AtLeast | BinOp::AtMost) {
            let lt = self.check_expr(left, None);
            let rt = self.check_expr(right, None);
            let numeric = (lt.ty.is_numeric() || matches!(lt.ty, Type::Error))
                && (rt.ty.is_numeric() || matches!(rt.ty, Type::Error));
            if !numeric {
                self.diags.push(
                    Diagnostic::error(
                        "E0356",
                        format!("`is {}` needs two numbers, but got {} and {}.", op_word(op), lt.ty.display(), rt.ty.display()),
                        span,
                    )
                    .with_explanation("Ordering (bigger/smaller) only makes sense for numbers."),
                );
            }
            return TypedExpr { expr: e_binary(op, left, right, span), ty: Type::Boolean };
        }
        // Boolean operators: strictly boolean (S-4, doc 03's open question
        // resolved for M0).
        if matches!(op, BinOp::And | BinOp::Or) {
            let lt = self.check_expr(left, Some(&Type::Boolean));
            let rt = self.check_expr(right, Some(&Type::Boolean));
            self.require_boolean(&lt, "`and`");
            self.require_boolean(&rt, "`or`");
            return TypedExpr { expr: e_binary(op, left, right, span), ty: Type::Boolean };
        }
        // Arithmetic: numeric; mixed promotes to decimal (S-1/S-3); `divided
        // by` always promotes (D-10).
        let lt = self.check_expr(left, None);
        let rt = self.check_expr(right, expected);
        if !lt.ty.is_numeric() && !matches!(lt.ty, Type::Error) {
            self.diags.push(
                Diagnostic::error(
                    "E0357",
                    format!("Arithmetic needs numbers, but the left side is {}.", lt.ty.display()),
                    lt.span(),
                )
                .with_explanation(format!("`{}` works on numbers and decimals.", op_word(op))),
            );
        }
        if !rt.ty.is_numeric() && !matches!(rt.ty, Type::Error) {
            self.diags.push(
                Diagnostic::error(
                    "E0357",
                    format!("Arithmetic needs numbers, but the right side is {}.", rt.ty.display()),
                    rt.span(),
                )
                .with_explanation(format!("`{}` works on numbers and decimals.", op_word(op))),
            );
        }
        let ty = match op {
            BinOp::Div => Type::Decimal,
            _ => {
                let l = if matches!(lt.ty, Type::Error) { Type::Number } else { lt.ty.clone() };
                let r = if matches!(rt.ty, Type::Error) { Type::Number } else { rt.ty.clone() };
                unify_numeric(&l, &r).unwrap_or(Type::Error)
            }
        };
        TypedExpr { expr: e_binary(op, left, right, span), ty }
    }

    fn require_boolean(&mut self, t: &TypedExpr, what: &str) {
        match &t.ty {
            Type::Boolean | Type::Error => {}
            Type::Text => {
                self.diags.push(
                    Diagnostic::error(
                        "E0358",
                        "A text is not a yes-or-no value.",
                        t.span(),
                    )
                    .with_explanation("`if`, `while`, `not`, and `and`/`or` need a condition that is true or false.")
                    .with_fix("compare it, like `size of name is greater than 0`"),
                );
            }
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0358",
                        format!("{what} needs a yes-or-no (boolean) value, but this is {}.", other.display()),
                        t.span(),
                    ),
                );
            }
        }
    }

    fn require_type(&mut self, t: &TypedExpr, want: &Type, what: &str, role: &str) {
        if matches!(t.ty, Type::Error) || matches!(want, Type::Error) {
            return;
        }
        let ok = numeric_or_same(want, &t.ty);
        if !ok {
            self.diags.push(
                Diagnostic::error(
                    "E0359",
                    format!("{what} needs {role} to be {}, but it is {}.", want.display(), t.ty.display()),
                    t.span(),
                ),
            );
        }
    }

    fn require_assignable(&mut self, want: &Type, got: &TypedExpr, what: &str) {
        if matches!(want, Type::Error) || matches!(got.ty, Type::Error) {
            return;
        }
        // The one implicit conversion: a number where a decimal is expected
        // (R-15's literal rule generalized to number→decimal widening reads;
        // decimal→number is NOT implicit — the teachable case is an error,
        // D-10's "annotated number accumulator receiving decimals" case).
        let ok = match (want, &got.ty) {
            (Type::Decimal, Type::Number) | (Type::Decimal, Type::NumericLit) => true,
            (Type::Number, Type::Decimal) | (Type::Number, Type::NumericLit) => false,
            (a, b) => a == b,
        };
        if !ok {
            self.diags.push(
                Diagnostic::error(
                    "E0360",
                    format!("{what} needs {}, but got {}.", want.display(), got.ty.display()),
                    got.span(),
                ),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Rebuild a binary expression node (sema preserves the AST shape; HIR owns
/// any desugaring).
fn e_binary(op: BinOp, left: &Expr, right: &Expr, span: Span) -> Expr {
    Expr::Binary {
        op,
        left: Box::new(left.clone()),
        right: Box::new(right.clone()),
        span,
    }
}

fn s_stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Make { span, .. }
        | Stmt::Set { span, .. }
        | Stmt::Change { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Stop { span }
        | Stmt::Next { span }
        | Stmt::GiveBack { span, .. }
        | Stmt::FailWith { span, .. }
        | Stmt::Attempt { span, .. }
        | Stmt::CheckThat { span, .. }
        | Stmt::ExprStmt { span, .. } => *span,
        Stmt::Repeat(r) => *repeat_span(r),
    }
}

fn repeat_span(r: &Repeat) -> &Span {
    match r {
        Repeat::Count { span, .. } | Repeat::While { span, .. } | Repeat::ForEach { span, .. } => span,
    }
}

fn body_gives_back(block: &ast::Block) -> bool {
    block.stmts.iter().any(|s| matches!(s, ast::Stmt::GiveBack { .. }))
}

fn op_word(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "plus",
        BinOp::Sub => "minus",
        BinOp::Mul => "times",
        BinOp::Div => "divided by",
        BinOp::DivEvenly => "divided evenly by",
        BinOp::Rem => "remainder of",
        BinOp::And => "and",
        BinOp::Or => "or",
        BinOp::Equal => "equal to",
        BinOp::NotEqual => "not equal to",
        BinOp::Greater => "greater than",
        BinOp::Less => "less than",
        BinOp::AtLeast => "at least",
        BinOp::AtMost => "at most",
    }
}

/// Numeric unification (S-3): number+decimal → decimal; equal types stay;
/// NumericLit bends either way (R-15).
fn unify_numeric(a: &Type, b: &Type) -> Option<Type> {
    match (a, b) {
        (Type::Error, _) | (_, Type::Error) => Some(Type::Error),
        (Type::Number, Type::Number) => Some(Type::Number),
        (Type::Decimal, Type::Decimal) => Some(Type::Decimal),
        (Type::NumericLit, other) | (other, Type::NumericLit) => {
            if other.is_numeric() {
                Some(if matches!(other, Type::NumericLit) { Type::Number } else { other.clone() })
            } else {
                None
            }
        }
        (Type::Number, Type::Decimal) | (Type::Decimal, Type::Number) => Some(Type::Decimal),
        _ if a == b => Some(a.clone()),
        _ => None,
    }
}

fn same_key_type(map: &Type, key: &Type) -> bool {
    match map {
        Type::Map(k, _) => unify_numeric(k, key).is_some() || **k == *key,
        _ => false,
    }
}

fn map_key_display(map: &Type) -> String {
    match map {
        Type::Map(k, _) => k.display(),
        _ => "?".into(),
    }
}

fn unify_opt(a: Option<Type>, b: Type) -> Type {
    match a {
        None => b,
        Some(prev) => match unify_numeric(&prev, &b) {
            Some(u) => u,
            None if prev == b => prev,
            None => Type::Error,
        },
    }
}

/// Acceptable when the types are equal, or both numeric-compatible.
fn numeric_or_same(want: &Type, got: &Type) -> bool {
    unify_numeric(want, got).is_some() || want == got
}

/// Simple did-you-mean: prefix/substring match against known names (full
/// edit-distance arrives with the M1 tooling pass).
fn did_you_mean(name: &str, candidates: &[String]) -> Vec<String> {
    let lower = name.to_lowercase();
    let mut hits: Vec<String> = candidates
        .iter()
        .filter(|c| {
            let cl = c.to_lowercase();
            cl.contains(&lower) || lower.contains(&cl)
        })
        .cloned()
        .collect();
    hits.sort();
    hits.truncate(3);
    hits
}

// ---------------------------------------------------------------------------
// Tests — one valid + one invalid program per rule; every diagnostic asserted
// by code, rendered with a dummy source file so failures read like user output.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lagom_parser::parse;

    /// Parse + check; panic with rendered diagnostics if any error appears.
    fn check_ok(src: &str) -> CheckedProgram {
        TEST_SRC.with(|s| *s.borrow_mut() = src.to_string());
        let (p, errs) = parse(src);
        assert!(
            errs.is_empty(),
            "parser rejected:\n{src}\n{}",
            errs.iter().map(render).collect::<Vec<_>>().join("\n")
        );
        let out = check(&p, src);
        assert_no_errors(src, &out.diags.items);
        out
    }

    /// Parse + check; assert at least one diagnostic with the given code.
    fn check_err(src: &str, code: &str) -> Vec<Diagnostic> {
        TEST_SRC.with(|s| *s.borrow_mut() = src.to_string());
        let (p, errs) = parse(src);
        let mut all = errs;
        let out = check(&p, src);
        all.extend(out.diags.items);
        assert!(
            all.iter().any(|d| d.code == code),
            "expected {code} for:\n{src}\ngot: {}",
            all.iter().map(render).collect::<Vec<_>>().join("\n")
        );
        all
    }

    fn render(d: &Diagnostic) -> String {
        let f = lagom_diagnostics::SourceFile::new("test.lagom", TEST_SRC.with(|s| s.borrow().clone()));
        lagom_diagnostics::render_student(&f, d)
    }

    // ----- 7.0.3 greedy split: nested calls keep their checked form -----

    /// `say greet who`: the word-run splits into say(greet(who)) — the
    /// argument is itself a checked call, not an unchecked multi-word name.
    /// (Regression: the pre-check clone returned here left the nested call
    /// untyped, silently evaluating to nothing on both backends.)
    #[test]
    fn effect_call_with_name_argument_splits_and_checks() {
        let out = check_ok(
            "function greet\n    takes text called name\n    returns a text\n    give back \"Hello, {name}!\"\n\nmake who equal to \"bo\"\nsay greet who\n",
        );
        // The main body's last statement is say(greet(who)): a Call whose
        // argument is itself a Call.
        let last = out.items.last().expect("a statement");
        let CheckedItem::Stmt(s) = last else { panic!("expected a statement") };
        let CheckedStmtKind::ExprStmt { expr } = &s.kind else { panic!("expected an effect statement") };
        let Expr::Call(outer) = &expr.expr else { panic!("expected a call, got {:?}", expr.expr) };
        assert_eq!(outer.callee.display(), "say");
        let arg = outer.first.as_ref().expect("say takes the greeting");
        let Expr::Call(inner) = arg.expr.as_ref() else { panic!("the argument must be a checked call, got {:?}", arg.expr) };
        assert_eq!(inner.callee.display(), "greet");
        let inner_arg = inner.first.as_ref().expect("greet takes the name");
        let Expr::Name { name: n, .. } = inner_arg.expr.as_ref() else { panic!("expected the name argument") };
        assert_eq!(n.display(), "who");
    }

    /// Interpolation keeps the checked form of its embedded calls too:
    /// `say "{greet who}"` is say(format(greet(who))), never an unchecked name.
    #[test]
    fn interpolation_keeps_checked_call_form() {
        let out = check_ok(
            "function greet\n    takes text called name\n    returns a text\n    give back \"Hello, {name}!\"\n\nmake who equal to \"bo\"\nsay \"{greet who}\"\n",
        );
        let last = out.items.last().expect("a statement");
        let CheckedItem::Stmt(s) = last else { panic!("expected a statement") };
        let CheckedStmtKind::ExprStmt { expr } = &s.kind else { panic!("expected an effect statement") };
        let Expr::Call(outer) = &expr.expr else { panic!("expected a call") };
        assert_eq!(outer.callee.display(), "say");
        let arg = outer.first.as_ref().expect("the formatted text");
        let Expr::Interp { parts, .. } = arg.expr.as_ref() else { panic!("expected interpolation, got {:?}", arg.expr) };
        let has_call = parts.iter().any(|p| matches!(p, ast::InterpPart::Expr(e) if matches!(e, Expr::Call(_))));
        assert!(has_call, "the interpolated call must stay a call: {parts:?}");
    }

    /// The greedy split reaches builtins through the same path: `say square
    /// root of 16` is say(square_root(16)) — two calls, one checked tree.
    #[test]
    fn greedy_split_reaches_builtins() {
        let out = check_ok("use math\nsay square root of 16\n");
        let last = out.items.last().expect("a statement");
        let CheckedItem::Stmt(s) = last else { panic!("expected a statement") };
        let CheckedStmtKind::ExprStmt { expr } = &s.kind else { panic!("expected an effect statement") };
        let Expr::Call(outer) = &expr.expr else { panic!("expected a call") };
        assert_eq!(outer.callee.display(), "say");
        let arg = outer.first.as_ref().expect("the root");
        let Expr::Call(inner) = arg.expr.as_ref() else { panic!("expected nested call, got {:?}", arg.expr) };
        assert_eq!(inner.callee.display(), "square root");
    }

    /// `remainder of a and b` (5.1/S-1) checks as the Rem operator over two
    /// numbers — the call form is type-checked like the infix form.
    #[test]
    fn remainder_of_call_form_checks() {
        check_ok("make total equal to 7\nmake rest equal to remainder of total and 2\n");
        check_err("make total equal to 7\nmake rest equal to remainder of total and \"x\"", "E0357");
    }

    thread_local! {
        static TEST_SRC: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }

    fn assert_no_errors(src: &str, diags: &[Diagnostic]) {
        let errs: Vec<Diagnostic> = diags.iter().filter(|d| d.severity == lagom_diagnostics::Severity::Error).cloned().collect();
        assert!(
            errs.is_empty(),
            "expected clean check for:\n{src}\ngot:\n{}",
            errs.iter().map(render).collect::<Vec<_>>().join("\n")
        );
    }

    // ----- name resolution (7.0.3) -----

    #[test]
    fn bindings_resolve_and_multiword_names_work() {
        check_ok("make top score equal to 10\nsay top score");
        check_ok("make changing total equal to 0\nset total to 5\nincrease total by 2");
    }

    #[test]
    fn greedy_split_callee_arg() {
        // `greet name` splits: callee `greet`, argument the binding `name`.
        check_ok("\nfunction greet\n    takes text called name\n    say name\n\nmake name equal to \"bo\"\ngreet name");
    }

    #[test]
    fn unknown_name_diagnoses() {
        check_err("say scoer", "E0344");
    }

    #[test]
    fn prefix_name_clash_rejected_at_declaration() {
        check_err("make top equal to 1\nmake top score equal to 2", "E0331");
    }

    #[test]
    fn exact_redeclaration_diagnoses() {
        check_err("make x equal to 1\nmake x equal to 2", "E0330");
    }

    // ----- mutability (7.2) -----

    #[test]
    fn immutable_rebind_diagnoses() {
        check_err("make score equal to 10\nset score to 5", "E0339");
    }    #[test]
    fn mutable_targets_pass() {
        check_ok("make changing score equal to 10\nset score to 5");
    }

    #[test]
    fn change_needs_numbers() {
        check_err("make changing t equal to \"hi\"\nincrease t by 1", "E0334");
    }

    // ----- types & arithmetic (D-10, R-15, S-1/S-3) -----

    #[test]
    fn number_is_the_literal_default() {
        let src = "make x equal to 5";
        let out = check_ok(src);
        match &out.items[0] {
            CheckedItem::Stmt(s) => match &s.kind {
                CheckedStmtKind::Make { value, .. } => assert!(matches!(value.ty, Type::Number)),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn literal_unifies_with_decimal_context() {
        // R-15: the accumulator case — `0` binds as decimal in a decimal list.
        check_ok("make scores equal to a list of 1.5, 0\nsay size of scores");
    }

    #[test]
    fn division_promotes_to_decimal() {
        let out = check_ok("make half equal to 5 divided by 2");
        match &out.items[0] {
            CheckedItem::Stmt(s) => match &s.kind {
                CheckedStmtKind::Make { value, .. } => assert!(matches!(value.ty, Type::Decimal)),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn mixed_arithmetic_promotes() {
        check_ok("make x equal to 1 plus 2.5");
    }

    #[test]
    fn annotated_number_receiving_text_diagnoses() {
        check_err("make n equal to \"hi\" of type number", "E0360");
    }

    #[test]
    fn arithmetic_on_text_diagnoses() {
        check_err("make x equal to \"a\" plus \"b\"", "E0357");
    }

    #[test]
    fn decimal_to_number_is_not_implicit() {
        check_err("\nfunction f\n    returns a number\n    give back 1.5", "E0360");
    }

    #[test]
    fn returns_inferred_when_absent() {
        // §7.8: `returns` is optional; the body's give back infers it (D-11).
        check_ok("\nfunction count down\n    takes number called n\n    if n is at most 0\n        give back 0\n    give back count down n minus 1");
    }

    // ----- booleans & comparisons (S-4, D-30, 8.3) -----

    #[test]
    fn no_truthiness_for_text() {
        let errs = check_err("make name equal to \"bo\"\nif name\n    say \"yes\"", "E0358");
        let d = errs.iter().find(|d| d.code == "E0358").unwrap();
        assert!(d.fix.is_some(), "the teaching diagnostic suggests the size comparison");
    }

    #[test]
    fn boolean_ops_strictly_boolean() {
        check_ok("if true and false or true\n    say \"y\"");
        check_err("if 1 and 2\n    say \"y\"", "E0358");
    }

    #[test]
    fn equality_needs_same_types() {
        check_err("if \"a\" is equal to 1\n    say \"no\"", "E0355");
    }

    #[test]
    fn structural_equality_of_collections_types() {
        check_ok("make a equal to a list of 1, 2\nmake b equal to a list of 1, 2\nif a is equal to b\n    say \"same\"");
    }

    #[test]
    fn ordering_needs_numbers() {
        check_err("if \"a\" is greater than \"b\"\n    say \"no\"", "E0356");
    }

    // ----- options (8.5, S-13) -----

    #[test]
    fn first_of_list_is_option() {
        // `result` is bound only by attempt tails (13.1) — a plain `if` sees
        // the option itself, which `say` prints as the value or `nothing`.
        check_ok("make scores equal to a list of 1, 2\nmake maybe equal to first of scores\nif maybe is nothing\n    say \"empty\"\notherwise\n    say maybe");
    }

    #[test]
    fn nothing_against_plain_value_diagnoses() {
        check_err("make n equal to 5\nif n is nothing\n    say \"no\"", "E0355");
    }

    #[test]
    fn is_something_works() {
        check_ok("make maybe equal to first of a list of \"x\"\nif maybe is something\n    say \"got one\"");
    }

    // ----- lists/maps/structs (7.6/7.11) -----

    #[test]
    fn list_element_mismatch_diagnoses() {
        check_err("make things equal to a list of 1, \"a\"", "E0346");
    }

    #[test]
    fn struct_roundtrip() {
        check_ok("structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\" and score 0\nsay name of p\nmake changing ps equal to a list of p\nset first of ps to p");
    }

    #[test]
    fn unknown_field_diagnoses() {
        check_err("structure player\n    has name of type text\n\nmake p equal to a player with nick \"bo\"", "E0348");
    }

    #[test]
    fn missing_field_diagnoses() {
        check_err("structure player\n    has name of type text\n    has score of type number\n\nmake p equal to a player with name \"bo\"", "E0349");
    }

    #[test]
    fn field_write_on_wrong_type_diagnoses() {
        check_err("make n equal to 5\nset n of p to 1", "E0343");
    }

    // ----- functions, calls, capability (13.1, S-6) -----

    #[test]
    fn call_arity_and_types() {
        check_ok("\nfunction add\n    takes number called a\n    takes number called b\n    returns a number\n    give back a plus b\n\nsay add 2 and 3");
        check_err("\nfunction add\n    takes number called a\n    returns a number\n    give back a\n\nsay add 2 and 3", "E0354");
    }

    #[test]
    fn recursion_resolves_file_relative() {
        check_ok("\nfunction count down\n    takes number called n\n    if n is at most 0\n        give back 0\n    give back count down n minus 1");
    }

    #[test]
    fn unhandled_can_fail_call_is_compile_error() {
        check_err("\nfunction divide\n    takes number called top\n    takes number called bottom\n    returns a number\n    can fail\n    give back top divided by bottom\n\nsay divide 10 and 0", "E0302");
    }

    #[test]
    fn attempt_handles_can_fail() {
        check_ok("\nfunction divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    if bottom is equal to 0\n        fail with \"cannot divide by zero\"\n    give back top divided by bottom\n\nattempt divide 10 and 0 if it fails then\n    say problem\notherwise\n    say result");
    }

    #[test]
    fn as_binding_and_propagation() {
        // G-14: the spec example's `returns` corrects to `a decimal` (D-10).
        check_ok("\nfunction divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    give back top divided by bottom\n\nattempt divide 1 and 2 as problem\n    say problem");
        // Propagation requires the enclosing function to declare can fail.
        check_err("\nfunction divide\n    takes number called top\n    returns a decimal\n    can fail\n    give back top\n\nfunction inner\n    attempt divide 1 and 2 and pass the problem on", "E0332");
    }

    #[test]
    fn bare_attempt_needs_can_fail_function() {
        check_err("\nfunction divide\n    takes number called top\n    returns a number\n    can fail\n    give back top\n\nmake x equal to attempt divide 1 and 2", "E0332");
    }

    #[test]
    fn fail_with_needs_can_fail() {
        check_err("function f\n    fail with \"no\"", "E0337");
        check_ok("function f\n    can fail\n    fail with \"yes\"");
    }

    #[test]
    fn returns_contract() {
        check_err("\nfunction f\n    returns a number", "E0333");
        // `give back` outside a function is the E0336 case; inside a function
        // without `returns`, the type is inferred (7.8/D-11).
        check_err("give back 1", "E0336");
    }

    #[test]
    fn conversion_can_fail_and_types() {
        check_ok("make answer equal to ask \"n?\"\nattempt number from answer if it fails then\n    say \"not a number\"\notherwise\n    say result");
        check_err("make answer equal to ask \"n?\"\nmake n equal to number from answer", "E0302");
        check_err("make n equal to number from 5", "E0359");
    }

    // ----- loops (7.5, S-12) & tests (5.1) -----

    #[test]
    fn all_three_loop_forms_check() {
        check_ok("repeat 3 times\n    say \"hi\"");
        check_ok("make changing n equal to 3\nrepeat while n is greater than 0\n    decrease n by 1");
        check_ok("make things equal to a list of 1, 2\nrepeat for each item, position in things\n    say position");
    }

    #[test]
    fn loop_var_scopes() {
        // Loop variables are immutable bindings (no `changing` form in the
        // frozen grammar); mutating one is the E0339 case.
        check_ok("make things equal to a list of 1\nrepeat for each item in things\n    say item");
        check_err(
            "make things equal to a list of 1\nrepeat for each item in things\n    increase item by 1",
            "E0339",
        );
    }

    #[test]
    fn for_each_needs_a_list() {
        check_err("repeat for each item in 5\n    say item", "E0338");
    }

    #[test]
    fn stop_next_need_a_loop() {
        check_err("stop", "E0335");
        check_err("next", "E0335");
        check_ok("repeat 3 times\n    stop");
        check_ok("repeat 3 times\n    next");
    }

    #[test]
    fn check_that_needs_boolean() {
        check_ok("test \"math\"\n    check that 2 plus 2 is equal to 4");
        check_err("\ntest \"bad\"\n    check that 5", "E0358");
    }

    // ----- modules (7.13, G-9) -----

    #[test]
    fn math_requires_use() {
        check_err("say square root of 16", "E0353");
        check_ok("use math\nsay square root of 16");
        check_ok("use math for square root\nsay square root of 16");
    }

    #[test]
    fn unknown_module_diagnoses() {
        check_err("use rocket", "E0340");
    }

    #[test]
    fn random_from_two_numbers() {
        check_ok("make roll equal to random from 1 to 6");
        check_err("make roll equal to random from 1", "E0354");
    }

    #[test]
    fn interpolation_checks_inner_exprs() {
        check_ok("make name equal to \"bo\"\nsay \"hello {name}, you have {2 plus 3} points\"");
        check_err("make name equal to \"bo\"\nsay \"hello {nope}\"", "E0344");
    }

    #[test]
    fn shadowing_attempt_names_lints() {
        // §13.1: shadowing `problem`/`result` inside attempt branches is
        // allowed with a lint (W0332), not an error.
        let out = check_ok(
            "\nfunction divide\n    takes number called top\n    returns a decimal\n    can fail\n    give back top\n\nattempt divide 1 as problem\n    make problem equal to \"x\"\n    say problem\notherwise\n    make result equal to 1\n    say result",
        );
        assert!(
            out.diags.items.iter().any(|d| d.code == "W0332"),
            "expected the shadow lint"
        );
    }
}

pub fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Int { span, .. }
        | Expr::Float { span, .. }
        | Expr::Text { span, .. }
        | Expr::Bool { span, .. }
        | Expr::Nothing { span }
        | Expr::Name { span, .. }
        | Expr::Interp { span, .. }
        | Expr::Group { span, .. }
        | Expr::Neg { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Not { span, .. }
        | Expr::ListLit { span, .. }
        | Expr::MapLit { span, .. }
        | Expr::PairLit { span, .. }
        | Expr::StructLit { span, .. }
        | Expr::AttemptExpr { span, .. } => *span,
        Expr::Call(c) => c.span,
    }
}
