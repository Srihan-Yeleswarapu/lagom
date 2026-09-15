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
    /// Checked block-lambda bodies keyed by the lambda's span (11.1).
    pub lambda_bodies: HashMap<Span, Vec<CheckedStmt>>,
}

#[derive(Debug)]
pub enum CheckedItem {
    Use { module: String, span: Span },
    Function(CheckedFunction),
    Structure(CheckedStructure),
    /// `kind` (7.12) — carried to HIR as the variant table.
    Kind(CheckedKind),
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
pub struct CheckedKind {
    pub name: String,
    /// Variants in declaration order: (variant name, fields (name, type, span), span).
    pub variants: Vec<(String, Vec<(String, Type, Span)>, Span)>,
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
    /// `match` (7.12): scrutinee + checked arms.
    Match {
        scrutinee: TypedExpr,
        arms: Vec<(CheckedPattern, Vec<CheckedStmt>)>,
        otherwise: Option<Vec<CheckedStmt>>,
    },
    ExprStmt {
        expr: TypedExpr,
    },
}

/// The checked form of one `when` pattern: types resolved against the
/// scrutinee, bindings ready to declare.
#[derive(Debug, Clone)]
pub enum CheckedPattern {
    /// A literal comparison (`when 0`, `when "quit"`).
    Literal { value: ast::PatternLiteral, span: Span },
    /// A tag test with destructured fields (`when a circle with radius r`).
    Variant { kind: String, variant: String, fields: Vec<(String, CheckedPattern)> },
    /// `something with value <pattern>` (8.5).
    Something { inner: Box<CheckedPattern>, span: Span },
    /// `a pair of <pattern> and <pattern>`.
    Pair { first: Box<CheckedPattern>, second: Box<CheckedPattern>, span: Span },
    /// A catch-all binding name.
    Binding { name: String, span: Span },
    Wildcard,
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
    /// A kind type (7.12) — the name of the `kind` declaration. Variant
    /// constructions (`a circle with radius 5`) have their kind's type.
    Kind(String),
    /// A function value (11.1): parameter types in order, then the return.
    /// Closures carry this type; `using` arguments must have it.
    Function(Vec<Type>, Box<Type>),
    /// The failure value's type at student level (S-10: a text message).
    Failure,
    /// A numeric literal before unification (R-15): compatible with Number and
    /// Decimal, defaults to Number.
    NumericLit,
    /// The type of a bare `nothing` (§8.5) — only comparable against options.
    NothingLit,
    /// The type parameter (12.1/12.2): `takes anything called x` / `takes
    /// some type called x`. The top type — every value assigns into it, and
    /// every occurrence in one function body is the same type (12.1's
    /// per-call-site unification).
    Anything,
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
            Type::Kind(n) => n.clone(),
            Type::Function(params, ret) => {
                let ps: Vec<String> = params.iter().map(|p| p.display()).collect();
                format!("a function from {} to {}", ps.join(" and "), ret.display())
            }
            Type::Failure => "a failure value".into(),
            Type::NumericLit => "number".into(),
            Type::NothingLit => "nothing".into(),
            Type::Anything => "anything".into(),
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
            TypeExpr::User(n) => {
                let name = n.display();
                if false {
                    Type::Struct(name)
                } else {
                    // A user name may denote a structure OR a kind (7.12);
                    // the Checker's tables decide at check time (see
                    // `user_type`), so `From` defaults to struct and the
                    // checker rewrites kind references where it sees them.
                    Type::Struct(name)
                }
            }
            TypeExpr::OptionT(t) => Type::Option(Box::new(t.as_ref().into())),
            TypeExpr::TypeParam => Type::Anything,
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

/// What kind of body is being checked (governs `gives back` rules).
#[derive(Clone, Copy, PartialEq)]
enum Body {
    Function,
    Test,
    Top,
}

pub struct Checker<'a> {
    src: &'a str,
    functions: HashMap<String, Callable>,
    structs: HashMap<String, CheckedStructure>,
    /// Type aliases (8.4): alias name → the fully-resolved real type.
    /// Transparent by design — `score` used anywhere a type is expected
    /// behaves exactly as the type it names.
    aliases: HashMap<String, Type>,
    /// Raw alias target spans, kept from pass 1 so cycle diagnostics can
    /// point at the alias line (drained by `resolve_alias_table`).
    pending_alias_spans: HashMap<String, Span>,
    /// `kind` name → its variant table (7.12). Variant payloads are keyed
    /// `(variant name, field name)`; a fieldless variant has no entries.
    kinds: HashMap<String, CheckedKind>,
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
    /// §8.5 flow-sensitive narrowing: `(name, inner type)` pairs currently
    /// narrowed to a definite value by an enclosing `is nothing` branch.
    /// Shadowed (pushed on branch entry, popped on exit) so nesting reads
    /// correctly; HIR/MIR need no instruction — the underlying binding still
    /// holds the option at runtime, and every use here types as the inner T.
    narrowed: Vec<(String, Type)>,
    /// The kind of body currently being checked (give-back rules).
    current_body: Body,
    /// Per-node types recorded while checking (keyed by node span).
    node_types: HashMap<Span, Type>,
    /// Call span → callee-name span (LOM provenance anchors, 26.5).
    callee_spans: HashMap<Span, Span>,
    /// Call spans that are flowing *reads* (`name of p`, `things at 2`) —
    /// rebuilt as ordinary calls, but classified precisely for HIR.
    read_spans: HashSet<Span>,
    /// Checked block-lambda bodies (11.1), keyed by the lambda's span — the
    /// side table HIR reads to lower the block form into its synthetic
    /// function (§11.1: Lagom's lambda is a full function).
    lambda_bodies: HashMap<Span, Vec<CheckedStmt>>,
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
        aliases: HashMap::new(),
        pending_alias_spans: HashMap::new(),
        kinds: HashMap::new(),
        used_modules: Vec::new(),
        scopes: vec![Scope { bindings: HashMap::new(), order: Vec::new() }],
        narrowed: Vec::new(),
        current_returns: None,
        current_can_fail: false,
        attempt_depth: 0,
        loop_depth: 0,
        current_body: Body::Top,
        node_types: HashMap::new(),
        callee_spans: HashMap::new(),
        read_spans: HashSet::new(),
        lambda_bodies: HashMap::new(),
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
        for name in ["square root", "floor", "bigger", "smaller"] {
            self.functions.insert(
                name.to_string(),
                Callable { params: vec![], ret: Type::Error, can_fail: false, origin: Origin::Math },
            );
        }
        // M1 stdlib surface (§11.2 combinators, docs/07's student text set,
        // §19.1 files/json modules). Arity registered here drives name
        // resolution only; the type rules live in `check_resolved_call`'s
        // dispatch table. Every name is derivable from the frozen call
        // grammar: multi-word callees (`open file`, `json map`), the closed
        // preposition set (`at`, `from`), and labeled `with` arguments.
        for name in ["map", "keep", "combine", "split", "contains", "sort"] {
            add(name, self);
        }
        // Two-argument builtins: their real arity participates in callee
        // resolution (`say contains "hello" and "ell"` must route both args
        // to `contains`, not leak them to `say`). `bigger`/`smaller` (G-27)
        // are two-argument too: `say bigger of 3 and 4` routes `3` and `4` to
        // the max call.
        for name in ["split", "contains", "combine", "bigger", "smaller"] {
            self.functions.insert(
                name.to_string(),
                Callable { params: vec![Type::Error, Type::Error], ret: Type::Error, can_fail: false, origin: Origin::Standard },
            );
        }
        // File ops carry real arities (G-25 uses them to decide whether a
        // one-run callee split's preps are positional arguments or reads).
        self.functions.insert(
            "open file".to_string(),
            Callable { params: vec![Type::Text], ret: Type::Error, can_fail: true, origin: Origin::Standard },
        );
        for (name, nparams) in [
            ("write file", 2usize),
            ("append file", 2usize),
            ("delete file", 1usize),
            ("file size", 1usize),
        ] {
            self.functions.insert(
                name.to_string(),
                Callable { params: vec![Type::Error; nparams], ret: Type::Error, can_fail: true, origin: Origin::Standard },
            );
        }
        self.functions.insert(
            "file exists".to_string(),
            Callable { params: vec![], ret: Type::Error, can_fail: false, origin: Origin::Standard },
        );
        self.functions.insert(
            "json".to_string(),
            Callable { params: vec![], ret: Type::Error, can_fail: true, origin: Origin::Standard },
        );
        self.functions.insert(
            "json text".to_string(),
            Callable { params: vec![], ret: Type::Error, can_fail: false, origin: Origin::Standard },
        );
        // `call f with x` — apply a closure value (11.1). Type rules live in
        // the dispatch table.
        self.functions.insert(
            "call".to_string(),
            Callable { params: vec![], ret: Type::Error, can_fail: false, origin: Origin::Standard },
        );
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
                ast::Item::Structure(s) => self.register_structure(s),
                ast::Item::Kind(k) => self.register_kind(k),
                ast::Item::TypeAlias(a) => self.register_type_alias(a),
                // Signatures register after the alias table resolves (see
                // below) so alias-typed params/returns are final.
                ast::Item::Function(_) => {}
                _ => {}
            }
        }
        // Aliases resolve order-free (7.13): all raw targets are registered
        // first, then each is flattened — so `a type called tally is a count`
        // may precede `a type called count is a number`. Signatures register
        // AFTER the table flattens, so alias-typed params/returns land in
        // the callables fully resolved.
        self.resolve_alias_table();
        for item in &program.items {
            if let ast::Item::Function(f) = item {
                self.register_function_sig(f);
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
                            .map(|f| (f.name.display(), self.user_type((&f.ty).into()), f.name.span))
                            .collect(),
                    }))
                }
                ast::Item::Kind(k) => {
                    let name = k.name.display();
                    let variants: Vec<(String, Vec<(String, Type, Span)>, Span)> = k
                        .variants
                        .iter()
                        .map(|v| {
                            (
                                v.name.display(),
                                v.fields
                                    .iter()
                                    .map(|(n, t, s)| (n.display(), self.user_type(t.into()), *s))
                                    .collect(),
                                v.span,
                            )
                        })
                        .collect();
                    items.push(CheckedItem::Kind(CheckedKind { name: name.clone(), variants }))
                }
                // Aliases carry no runtime or HIR presence (transparent,
                // 8.4): they were fully consumed by the type tables.
                ast::Item::TypeAlias(_) => {}
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
            lambda_bodies: std::mem::take(&mut self.lambda_bodies),
        }
    }

    fn register_use(&mut self, u: &ast::UseDecl) {
        let module = u.module.display();
        if !matches!(module.as_str(), "math" | "random" | "standard" | "files" | "json") {
            self.diags.push(
                Diagnostic::error(
                    "E0340",
                    format!("There is no module called `{module}`."),
                    u.module.span,
                )
                .with_explanation("At M1 the modules are: `standard` (always in scope), `math`, `random`, `files`, and `json`.")
                .with_fix("use math"),
            );
        } else if !self.is_used(&module) {
            self.used_modules.push(module);
        }
    }

    /// `a type called score is a number` (8.4) — pass 1: record the raw
    /// target under the alias name (resolution flattens chains afterwards).
    /// Duplicate names are a teaching error (E0371); an alias that shadows a
    /// real type (structure/kind) is E0372.
    fn register_type_alias(&mut self, a: &ast::TypeAliasDecl) {
        let name = a.name.display();
        if self.aliases.contains_key(&name) || self.structs.contains_key(&name) || self.kinds.contains_key(&name)
        {
            self.diags.push(
                Diagnostic::error(
                    "E0371",
                    format!("There is already a type called `{name}`."),
                    a.name.span,
                )
                .with_explanation("Each type name can mean only one type — a new name needs new words."),
            );
            return;
        }
        self.pending_alias_spans.insert(name.clone(), a.span);
        self.aliases.insert(name, (&a.ty).into());
    }

    /// Flatten the alias table (order-free resolution, 7.13): every target
    /// resolves to a fully-resolved type with cycle detection. A cycle
    /// (`a type called a is a b` + `a type called b is a a`) is a teaching
    /// error (E0372, the self-reference rule applied transitively) and the
    /// cyclic aliases are removed — uses of them then report the unknown
    /// type name, the actual root cause.
    fn resolve_alias_table(&mut self) {
        // Spans for cycle diagnostics (borrowed from the AST during pass 1).
        let alias_spans: HashMap<String, Span> = self.pending_alias_spans.drain().collect();
        let raw: Vec<(String, Type)> = self.aliases.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        // Unknown targets (E0373): the words after `is a` must name a type
        // that exists — a builtin, a structure, a kind, or another alias.
        // Diagnosed on the alias line, where the mistake is; the alias is
        // removed so its uses report the unknown name, not a phantom type.
        let mut unknown: Vec<String> = Vec::new();
        for (name, ty) in &raw {
            if let Some(missing) = unknown_alias_ref(ty, &raw) {
                if let Some(span) = alias_spans.get(name) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0373",
                            format!("There is no type called `{missing}` — `a type called … is a …` needs a type that exists."),
                            *span,
                        )
                        .with_explanation("A type alias gives a new name to a type you already have — the words after `is a` must name one (like `number`, `text`, or a `structure` you made)."),
                    );
                }
                unknown.push(name.clone());
            }
        }
        for name in &unknown {
            self.aliases.remove(name);
        }
        let raw: Vec<(String, Type)> = raw.into_iter().filter(|(n, _)| !unknown.contains(n)).collect();
        let mut resolved: HashMap<String, Type> = HashMap::new();
        let mut cyclic: Vec<String> = Vec::new();
        for (name, _) in &raw {
            match self.resolve_one_alias(name, &raw, &mut resolved, &mut Vec::new()) {
                Ok(ty) => {
                    resolved.insert(name.clone(), ty);
                }
                Err(()) => cyclic.push(name.clone()),
            }
        }
        if !cyclic.is_empty() {
            // Diagnose every alias on the cycle, deduped.
            let mut seen: Vec<String> = Vec::new();
            for name in &cyclic {
                if seen.contains(name) {
                    continue;
                }
                seen.push(name.clone());
                if let Some(span) = alias_spans.get(name) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0372",
                            format!("A type cannot be defined in terms of itself: `{name}` is part of a circle of type definitions."),
                            *span,
                        )
                        .with_explanation("A type alias gives a new name to a type you already have — following the names after `is a` must reach a real type, not come back around."),
                    );
                }
            }
            for name in cyclic {
                self.aliases.remove(&name);
            }
        }
        for (k, v) in resolved {
            if self.aliases.contains_key(&k) {
                self.aliases.insert(k, v);
            }
        }
    }

    /// Walk one alias to its fully-resolved type. `stack` is the chain of
    /// names being resolved — re-visiting one is the cycle.
    fn resolve_one_alias(
        &self,
        name: &str,
        raw: &[(String, Type)],
        resolved: &mut HashMap<String, Type>,
        stack: &mut Vec<String>,
    ) -> Result<Type, ()> {
        if let Some(ty) = resolved.get(name) {
            return Ok(ty.clone());
        }
        if stack.iter().any(|s| s == name) {
            return Err(());
        }
        let Some((_, ty)) = raw.iter().find(|(n, _)| n == name) else {
            // Not an alias: the name must denote a structure or kind (the
            // registration check already ruled out unknown names — anything
            // else is an earlier-error recovery; error type is fine).
            return Ok(Type::Struct(name.to_string()));
        };
        stack.push(name.to_string());
        let out = self.resolve_type_alias_refs(ty.clone(), raw, resolved, stack)?;
        stack.pop();
        Ok(out)
    }

    /// Resolve every alias reference inside one raw target type.
    fn resolve_type_alias_refs(
        &self,
        t: Type,
        raw: &[(String, Type)],
        resolved: &mut HashMap<String, Type>,
        stack: &mut Vec<String>,
    ) -> Result<Type, ()> {
        Ok(match t {
            Type::Struct(name)
                if raw.iter().any(|(n, _)| *n == name)
                    && !self.structs.contains_key(&name)
                    && !self.kinds.contains_key(&name) =>
            {
                let inner = self.resolve_one_alias(&name, raw, resolved, stack)?;
                inner
            }
            Type::Struct(name) if self.kinds.contains_key(&name) => Type::Kind(name),
            Type::List(e) => Type::List(Box::new(self.resolve_type_alias_refs(*e, raw, resolved, stack)?)),
            Type::Map(k, v) => Type::Map(
                Box::new(self.resolve_type_alias_refs(*k, raw, resolved, stack)?),
                Box::new(self.resolve_type_alias_refs(*v, raw, resolved, stack)?),
            ),
            Type::Pair(a, b) => Type::Pair(
                Box::new(self.resolve_type_alias_refs(*a, raw, resolved, stack)?),
                Box::new(self.resolve_type_alias_refs(*b, raw, resolved, stack)?),
            ),
            Type::Option(e) => Type::Option(Box::new(self.resolve_type_alias_refs(*e, raw, resolved, stack)?)),
            Type::Function(ps, r) => Type::Function(
                ps.into_iter().map(|p| self.resolve_type_alias_refs(p, raw, resolved, stack)).collect::<Result<Vec<_>, ()>>()?,
                Box::new(self.resolve_type_alias_refs(*r, raw, resolved, stack)?),
            ),
            other => other,
        })
    }

    fn register_function_sig(&mut self, f: &ast::FunctionDecl) {
        let name = f.name.display();
        // Kinds are registered in this same pass; rewrite kind-typed params
        // when the kind declaration precedes the function (7.13: order-free
        // file resolution is repaired by the same rewrite in check_function).
        let params: Vec<Type> = f.params.iter().map(|p| self.user_type((&p.ty).into())).collect();
        let ret = f.returns.as_ref().map(|t| self.user_type(t.into())).unwrap_or(Type::Error);
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
            .map(|f| (f.name.display(), self.user_type((&f.ty).into()), f.name.span))
            .collect();
        self.structs.insert(
            name,
            CheckedStructure { name: s.name.display(), fields: fields.clone() },
        );
    }

    /// `kind` registration (7.12). A kind name is distinct from structure
    /// names (one namespace for user types in the student surface would be
    /// friendlier, but the frozen grammar keys constructions on the shape of
    /// the line, not the name — keeping two tables mirrors that). A variant
    /// name may not collide with another variant of the same kind.
    fn register_kind(&mut self, k: &ast::KindDecl) {
        let name = k.name.display();
        if self.kinds.contains_key(&name) || self.structs.contains_key(&name) {
            self.diags.push(
                Diagnostic::error(
                    "E0341",
                    format!("There is already a type called `{name}`."),
                    k.name.span,
                ),
            );
            return;
        }
        let mut variants: Vec<(String, Vec<(String, Type, Span)>, Span)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for v in &k.variants {
            let vname = v.name.display();
            if seen.contains(&vname) {
                self.diags.push(
                    Diagnostic::error(
                        "E0361",
                        format!("The kind `{name}` already has a variant called `{vname}`."),
                        v.name.span,
                    )
                    .with_explanation("A variant's name is how `match` tells the kinds apart — each variant needs its own name."),
                );
                continue;
            }
            seen.push(vname.clone());
            let fields: Vec<(String, Type, Span)> = v
                .fields
                .iter()
                .map(|(n, t, s)| (n.display(), t.into(), *s))
                .collect();
            variants.push((vname, fields, v.span));
        }
        self.kinds.insert(
            name,
            CheckedKind { name: k.name.display(), variants: variants.clone() },
        );
    }

    /// Field types of one variant of one kind.
    fn variant_fields(&self, kind: &str, variant: &str) -> Option<Vec<(String, Type)>> {
        self.kinds
            .get(kind)
            .and_then(|k| k.variants.iter().find(|(n, _, _)| n == variant))
            .map(|(_, fs, _)| fs.iter().map(|(n, t, _)| (n.clone(), t.clone())).collect())
    }

    /// Does the user-type name denote a kind? Builtins pass through; only a
    /// `Type::Struct(name)` whose name is a registered `kind` is rewritten.
    fn user_type(&self, t: Type) -> Type {
        match t {
            // Type aliases (8.4) are transparent: the alias name resolves to
            // its fully-resolved real type (the table was resolved after
            // registration, so chains are already flat and the recursion
            // terminates). Shadowing a *structure/kind* name is diagnosed at
            // registration, so a raw `Type::Struct(name)` here is a real
            // struct/kind reference, never an alias.
            Type::Struct(name) if self.aliases.contains_key(&name) => self
                .aliases
                .get(&name)
                .cloned()
                .unwrap_or(Type::Error),
            Type::Struct(name) if self.kinds.contains_key(&name) => Type::Kind(name),
            // Alias/kind references resolve at any depth — `a list of score`
            // and `a list of shape` are types the same way their elements
            // are.
            Type::List(e) => Type::List(Box::new(self.user_type(*e))),
            Type::Map(k, v) => Type::Map(Box::new(self.user_type(*k)), Box::new(self.user_type(*v))),
            Type::Pair(a, b) => Type::Pair(Box::new(self.user_type(*a)), Box::new(self.user_type(*b))),
            Type::Option(e) => Type::Option(Box::new(self.user_type(*e))),
            Type::Function(ps, r) => Type::Function(
                ps.into_iter().map(|p| self.user_type(p)).collect(),
                Box::new(self.user_type(*r)),
            ),
            other => other,
        }
    }

    fn check_function(&mut self, f: &ast::FunctionDecl) -> CheckedFunction {
        let params: Vec<CheckedParam> = f
            .params
            .iter()
            .map(|p| {
                let raw: Type = (&p.ty).into();
                CheckedParam {
                    name: p.name.display(),
                    ty: self.user_type(raw),
                    span: p.name.span,
                }
            })
            .collect();
        let ret = f.returns.as_ref().map(|t| {
            let raw: Type = t.into();
            self.user_type(raw)
        });
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
                        format!("This function promises to gives back a {} but might not.", rt.display()),
                        f.span,
                    )
                    .with_explanation("Every path through a function with a `returns` clause must end in `gives back <value>`.")
                    .with_fix(format!("add `gives back <a {}>` on every path", rt.display())),
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
        // §8.5 narrowing wins first: inside an `otherwise` branch of an
        // `is nothing` test, the name reads as the option's inner type.
        for (n, ty) in self.narrowed.iter().rev() {
            if n == name {
                return Some(Binding { ty: ty.clone(), mutable: false, span: Span::default() });
            }
        }
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.bindings.get(name) {
                return Some(b.clone());
            }
        }
        None
    }

    /// The inner type when `name` currently holds an option (8.5 narrowing).
    fn option_inner(&self, name: &str) -> Option<Type> {
        let b = self.lookup(name)?;
        match b.ty {
            Type::Option(inner) => Some((*inner).clone()),
            _ => None,
        }
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
                // The annotation resolves aliases (8.4) like every type
                // position — `of type score` and `of type number` are one.
                let expected = annotation.as_ref().map(|t| self.user_type(t.into()));
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
                // §8.5 flow-sensitive narrowing: when a branch condition is
                // `<name> is nothing`, the `otherwise` branch sees the name as
                // its option's inner type — no unwrap ceremony at student
                // level. Pushed only around the otherwise body; nested
                // conditions shadow correctly via the stack.
                let narrowed = otherwise.as_ref().and_then(|_| {
                    branches.first().and_then(|(cond, _)| {
                        // The parser desugars `x is nothing` to `x == nothing`.
                        if let ast::Expr::Binary { op: ast::BinOp::Equal, left, right, .. } = cond {
                            if let (ast::Expr::Name { name, .. }, ast::Expr::Nothing { .. }) =
                                (left.as_ref(), right.as_ref())
                            {
                                let full = name.display();
                                return self.option_inner(&full).map(|inner| (full, inner));
                            }
                        }
                        None
                    })
                });
                let out_otherwise = otherwise.as_ref().map(|b| {
                    if let Some((name, inner)) = narrowed.clone() {
                        self.narrowed.push((name, inner));
                    }
                    let out = self.check_block_stmts(b);
                    if narrowed.is_some() {
                        self.narrowed.pop();
                    }
                    out
                });
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
                    self.require_assignable(rt, &vt, "`gives back`");
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
                        Diagnostic::error("E0336", "`gives back` only works inside a function.", *span)
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
            Stmt::Match { scrutinee, arms, otherwise, span } => {
                self.check_match(scrutinee, arms, otherwise, *span)
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

    /// `match <expr>` (7.12/7.15): one scope, arms check patterns against
    /// the scrutinee's type, exhaustiveness over kinds and options.
    fn check_match(&mut self, scrutinee: &Expr, arms: &[(ast::Pattern, ast::Block)], otherwise: &Option<ast::Block>, span: Span) -> CheckedStmt {
        let st = self.check_expr(scrutinee, None);
        // A `match` must have at least one arm or an `otherwise`.
        let mut out_arms: Vec<(CheckedPattern, Vec<CheckedStmt>)> = Vec::new();
        let mut matched_variant: Option<(String, String)> = None;
        for (pat, block) in arms {
            let cp = self.check_pattern(pat, &st.ty);
            if let CheckedPattern::Variant { kind, variant, .. } = &cp {
                matched_variant = Some((kind.clone(), variant.clone()));
            }
            // All arm bindings live in one scope, dropped after the match —
            // the student model ("the names live inside the when").
            self.push_scope();
            self.declare_pattern_bindings(&cp);
            let body = self.check_block_stmts(block);
            self.pop_scope();
            out_arms.push((cp, body));
        }
        let out_otherwise = otherwise.as_ref().map(|b| {
            self.push_scope();
            let body = self.check_block_stmts(b);
            self.pop_scope();
            body
        });
        self.check_exhaustive(&st, &out_arms, otherwise.is_some(), matched_variant.as_ref(), span);
        CheckedStmt {
            kind: CheckedStmtKind::Match { scrutinee: st, arms: out_arms, otherwise: out_otherwise },
            span,
        }
    }

    /// Check one pattern against the scrutinee type; returns the checked
    /// pattern with binding types resolved.
    fn check_pattern(&mut self, pat: &ast::Pattern, scrutinee: &Type) -> CheckedPattern {
        match pat {
            ast::Pattern::Literal { value, span } => {
                // The literal must be comparable to the scrutinee (D-30's rule
                // applied to patterns): numbers to numbers, text to text, …
                let lit_ty = match value {
                    ast::PatternLiteral::Int(_) => Type::Number,
                    ast::PatternLiteral::Float(_) => Type::Decimal,
                    ast::PatternLiteral::Text(_) => Type::Text,
                    ast::PatternLiteral::Bool(_) => Type::Boolean,
                    ast::PatternLiteral::Nothing => Type::NothingLit,
                };
                let ok = match (&lit_ty, scrutinee) {
                    (Type::NothingLit, other) => matches!(other, Type::Option(_)),
                    (a, b) => unify_numeric(a, b).is_some() || a == b || matches!(b, Type::Error),
                };
                if !ok {
                    self.diags.push(
                        Diagnostic::error(
                            "E0362",
                            format!("This pattern matches {} but the matched value is {}.", lit_ty.display(), scrutinee.display()),
                            *span,
                        )
                        .with_explanation("A `when` pattern must match the type of the value in `match`."),
                    );
                }
                CheckedPattern::Literal { value: value.clone(), span: *span }
            }
            ast::Pattern::Name { name } => {
                let full = name.display();
                // A variant name of the scrutinee's kind binds no fields —
                // it is a tag test. Otherwise it is a binding pattern.
                if let Type::Kind(kname) = scrutinee {
                    if let Some((vname, fields, vspan)) = self
                        .kinds
                        .get(kname)
                        .and_then(|k| k.variants.iter().find(|(n, _, _)| n == &full).cloned())
                    {
                        if !fields.is_empty() {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0363",
                                    format!("`{vname}` carries values — match them: `when a {vname} with {}`.",
                                        fields.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>().join(" and ")),
                                    vspan,
                                ),
                            );
                        }
                        return CheckedPattern::Variant { kind: kname.clone(), variant: vname, fields: Vec::new() };
                    }
                }
                CheckedPattern::Binding { name: full, span: name.span }
            }
            ast::Pattern::Variant { name, fields, span } => {
                let vname = name.display();
                let (kname, def_fields): (String, Vec<(String, Type)>) = match scrutinee {
                    Type::Kind(k) => match self.variant_fields(k, &vname) {
                        Some(fs) => (k.clone(), fs),
                        // The kind exists but has no such variant (E0367),
                        // listing the variants it does have.
                        None => {
                            let have = self
                                .kinds
                                .get(k)
                                .map(|kk| {
                                    kk.variants.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>().join(", ")
                                })
                                .unwrap_or_default();
                            self.diags.push(
                                Diagnostic::error(
                                    "E0367",
                                    format!("There is no variant called `{vname}`."),
                                    name.span,
                                )
.with_explanation("A variant pattern names one of the kind's variants: `kind shape` + newline + `is a circle with radius of type number` makes `when a circle with radius r` legal.")
                                .with_note(format!("`{k}`'s variants are: {have}")),
                            );
                            (k.clone(), Vec::new())
                        }
                    },
                    Type::Error => (String::new(), Vec::new()),
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                "E0363",
                                format!("`{vname}` is a variant pattern, but the matched value is {}.", other.display()),
                                *span,
                            ),
                        );
                        (String::new(), Vec::new())
                    }
                };
                let mut out_fields: Vec<(String, CheckedPattern)> = Vec::new();
                for (fname, sub) in fields {
                    let fs = fname.display();
                    match def_fields.iter().find(|(n, _)| n == &fs) {
                        Some((_, fty)) => {
                            let sub_checked = self.check_pattern(sub, fty);
                            out_fields.push((fs, sub_checked));
                        }
                        None => {
                            let field_list = def_fields.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(", ");
                            self.diags.push(
                                Diagnostic::error(
                                    "E0364",
                                    format!("`{vname}` has no field called `{fs}`."),
                                    fname.span,
                                )
                                .with_note(format!("its fields are: {field_list}")),
                            );
                        }
                    }
                }
                CheckedPattern::Variant { kind: kname, variant: vname, fields: out_fields }
            }
            ast::Pattern::Something { inner, span } => {
                let inner_ty = match scrutinee {
                    Type::Option(t) => (**t).clone(),
                    Type::Error => Type::Error,
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                "E0365",
                                format!("`something with value` matches an option, but the matched value is {}.", other.display()),
                                *span,
                            )
                            .with_explanation("Only an option (`a … or nothing`) can be `something` or `nothing` — 8.5."),
                        );
                        Type::Error
                    }
                };
                let inner_checked = self.check_pattern(inner, &inner_ty);
                CheckedPattern::Something { inner: Box::new(inner_checked), span: *span }
            }
            ast::Pattern::Pair { first, second, span } => {
                let (ft, st2) = match scrutinee {
                    Type::Pair(a, b) => ((**a).clone(), (**b).clone()),
                    Type::Error => (Type::Error, Type::Error),
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                "E0365",
                                format!("`a pair of … and …` matches a pair, but the matched value is {}.", other.display()),
                                *span,
                            ),
                        );
                        (Type::Error, Type::Error)
                    }
                };
                let f = self.check_pattern(first, &ft);
                let s = self.check_pattern(second, &st2);
                CheckedPattern::Pair { first: Box::new(f), second: Box::new(s), span: *span }
            }
            ast::Pattern::Wildcard => CheckedPattern::Wildcard,
        }
    }

    /// Declare a checked pattern's bindings in the current scope.
    fn declare_pattern_bindings(&mut self, pat: &CheckedPattern) {
        match pat {
            CheckedPattern::Binding { name, span } => {
                self.declare(name, Type::Error, false, *span);
            }
            CheckedPattern::Variant { fields, .. } => {
                for (_, sub) in fields {
                    self.declare_pattern_bindings(sub);
                }
            }
            CheckedPattern::Something { inner, .. } => self.declare_pattern_bindings(inner),
            CheckedPattern::Pair { first, second, .. } => {
                self.declare_pattern_bindings(first);
                self.declare_pattern_bindings(second);
            }
            CheckedPattern::Literal { .. } | CheckedPattern::Wildcard => {}
        }
    }

    /// Exhaustiveness (7.12: "a match that misses a case is a compile error
    /// with the missing case named"). Checks:
    /// - kind scrutinees: every variant covered (or `otherwise`);
    /// - option scrutinees: `nothing` and `something` covered (or `otherwise`);
    /// - other types: a catch-all binding/wildcard arm or `otherwise` is
    ///   required — a literal-only match on a number, say, can never be
    ///   exhaustive and must say `otherwise`.
    fn check_exhaustive(
        &mut self,
        scrutinee: &TypedExpr,
        arms: &[(CheckedPattern, Vec<CheckedStmt>)],
        has_otherwise: bool,
        _matched_variant: Option<&(String, String)>,
        span: Span,
    ) {
        if has_otherwise {
            return;
        }
        let missing = match &scrutinee.ty {
            Type::Kind(kname) => {
                let Some(kind) = self.kinds.get(kname) else { return };
                let mut missing: Vec<String> = Vec::new();
                for (vname, _, _) in &kind.variants {
                    let covered = arms.iter().any(|(p, _)| match p {
                        CheckedPattern::Variant { variant, .. } => variant == vname,
                        CheckedPattern::Binding { name, .. } => name == vname,
                        _ => false,
                    });
                    if !covered {
                        missing.push(vname.clone());
                    }
                }
                if missing.is_empty() {
                    None
                } else {
                    Some(format!(
                        "this `match` misses {} of the kind `{kname}`: {}",
                        missing.len(),
                        missing.join(", ")
                    ))
                }
            }
            Type::Option(_) => {
                let has_nothing = arms.iter().any(|(p, _)| matches!(p, CheckedPattern::Literal { value: ast::PatternLiteral::Nothing, .. }));
                let has_something = arms.iter().any(|(p, _)| matches!(p, CheckedPattern::Something { .. }));
                let mut missing: Vec<String> = Vec::new();
                if !has_nothing {
                    missing.push("nothing".into());
                }
                if !has_something {
                    missing.push("something with value …".into());
                }
                if missing.is_empty() { None } else { Some(format!("this `match` misses: {}", missing.join(", "))) }
            }
            // A catch-all name or `otherwise` is required for every other
            // type: literal-only matches can never cover all values. A full
            // pair destructure (`a pair of x and y`) also covers every pair.
            _ => {
                let has_catch_all = arms.iter().any(|(p, _)| {
                    matches!(
                        p,
                        CheckedPattern::Binding { .. } | CheckedPattern::Wildcard | CheckedPattern::Pair { .. }
                    )
                });
                if has_catch_all { None } else { Some("this `match` can miss values — add an `otherwise` arm".into()) }
            }
        };
        if let Some(what) = missing {
            self.diags.push(
                Diagnostic::error("E0360", what, span)
                    .with_explanation("A `match` must handle every case. Add the missing `when` arm, or an `otherwise` arm for everything else.")
                    .with_fix("otherwise\n    …"),
            );
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
                    // G-26: the readable `?` forwards the success value — the
                    // statements after the propagate read it as `result`
                    // (13.1's attempt-flowing reading; the propagation
                    // short-circuits so `result` only exists on success).
                    self.declare("result", inner.ty.clone(), false, inner.span());
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
                    .with_fix(format!("make changing {base_name} equal to …"))
                    // M2: the causal footer — why the declaration is the fix
                    // (deleting the mutation would also silence the error and
                    // break the program; the fix must address the cause).
                    .with_fix_why("`changing` is what makes a binding mutable — only a `changing` binding accepts `set`, `increase`, or `decrease`"),
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
            Expr::Name { name, span } => self.check_name_expr(name, *span, expected),
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
                        )
                        .with_explanation("A leading `-` turns a number negative — the value after it must be a number."),
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
            Expr::StructLit { name, fields, span } => {
                // §7.12 construction is uniform at the parser level: a
                // structure literal and a variant construction share the `a
                // <name> with …` shape. Sema reclassifies against the tables.
                let sname = name.display();
                if !self.structs.contains_key(&sname)
                    && self.kinds.values().any(|k| k.variants.iter().any(|(n, _, _)| n == &sname))
                {
                    let owned: Vec<(Name, Expr)> = fields.clone();
                    self.check_variant_lit(name, &owned, *span)
                } else {
                    self.check_struct_lit(name, fields, *span, e)
                }
            }
            Expr::VariantLit { name, fields, span } => self.check_variant_lit(name, fields, *span),
            Expr::Lambda { params, body, span } => {
                self.check_lambda_expected(params, body, expected, *span)
            }
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
            Expr::SomeValue { value, span } => {
                // The option construction (8.5/G-21): `something with value v`.
                // The value binds at additive level; the result is an option of
                // the value's type — unless an expected option type tells us
                // the inner type (R-15's literal-unification analog).
                let inner_ty = match expected {
                    Some(Type::Option(t)) => (**t).clone(),
                    _ => self.check_expr(value, None).ty,
                };
                let inner = self.check_expr(value, Some(&inner_ty));
                if !matches!(inner.ty, Type::Error)
                    && !matches!(inner_ty, Type::Error)
                    && inner.ty != inner_ty
                {
                    self.diags.push(
                        Diagnostic::error(
                            "E0365",
                            format!(
                                "`something with value` was expected to hold a {}, but this value is a {}.",
                                inner_ty.display(),
                                inner.ty.display()
                            ),
                            *span,
                        )
                        .with_explanation("The value inside `something with value` must match the option's inner type — 8.5."),
                    );
                }
                TypedExpr { expr: e.clone(), ty: Type::Option(Box::new(inner_ty)) }
            }
            Expr::Call(call) => self.check_call(call, expected),
        }
    }

    /// Variant construction (7.12): `a circle with radius 5`. The name must
    /// be a variant of exactly one known kind; the result has the kind's type.
    fn check_variant_lit(&mut self, name: &Name, fields: &[(Name, Expr)], span: Span) -> TypedExpr {
        let vname = name.display();
        // Find the kind that owns this variant name.
        let owner: Option<(String, Vec<(String, Type)>)> = self
            .kinds
            .iter()
            .find_map(|(kname, k)| {
                k.variants
                    .iter()
                    .find(|(n, _, _)| n == &vname)
                    .map(|(_, fs, _)| (kname.clone(), fs.iter().map(|(n, t, _)| (n.clone(), t.clone())).collect()))
            });
        let Some((kname, def_fields)) = owner else {
            self.diags.push(
                Diagnostic::error(
                    "E0367",
                    format!("There is no variant called `{vname}`."),
                    name.span,
                )
                .with_explanation("A variant is built with its kind's variants: `kind shape ⏎ is a circle with radius of type number` makes `a circle with radius 5` legal."),
            );
            return TypedExpr { expr: Expr::Name { name: name.clone(), span }, ty: Type::Error };
        };
        let mut out_fields: Vec<(Name, Expr)> = Vec::new();
        for (fname, fexpr) in fields {
            let fs = fname.display();
            let ft = self.check_expr(fexpr, def_fields.iter().find(|(n, _)| n == &fs).map(|(_, t)| t));
            match def_fields.iter().find(|(n, _)| n == &fs) {
                Some((_, fty)) => {
                    let holder = TypedExpr { expr: fexpr.clone(), ty: ft.ty.clone() };
                    self.require_assignable(fty, &holder, &format!("the {vname}'s `{fs}` field"));
                    out_fields.push((fname.clone(), ft.expr));
                }
                None => {
                    let field_list = def_fields.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(", ");
                    self.diags.push(
                        Diagnostic::error(
                            "E0364",
                            format!("`{vname}` has no field called `{fs}`."),
                            fname.span,
                        )
                        .with_note(format!("its fields are: {field_list}")),
                    );
                    out_fields.push((fname.clone(), ft.expr));
                }
            }
        }
        // Missing fields are a compile error (same rule as structures).
        for (fname, fty) in &def_fields {
            if !out_fields.iter().any(|(n, _)| &n.display() == fname) {
                self.diags.push(
                    Diagnostic::error(
                        "E0368",
                        format!("A `{vname}` needs its `{fname}` field: `a {vname} with {fname} …`."),
                        span,
                    )
                    .with_note(format!("the field is {}", fty.display())),
                );
            }
        }
        let rebuilt = Expr::VariantLit { name: name.clone(), fields: out_fields, span };
        TypedExpr { expr: rebuilt, ty: Type::Kind(kname) }
    }

    /// A lambda (11.1): parameter names bound in a fresh scope, the body
    /// checked there. The type is `a function from … to …`. A parameter-less
    /// The deferred-lambda entry: `lam` is a desugared `Expr::Lambda` (from
    /// `desugar_using`/`desugar_where`); check it with the use-site's expected
    /// function type.
    fn check_lambda_expected_lambda(&mut self, lam: &Expr, expected: Option<&Type>) -> TypedExpr {
        match lam {
            Expr::Lambda { params, body, span } => self.check_lambda_expected(params, body, expected, *span),
            // A bare function name (`using double`) is a plain value read.
            other => self.check_expr(other, expected),
        }
    }

    /// The expected-parameter path: when the use site knows the function type
    /// (`map a list of 1, 2 using double` — 11.2), each parameter's type comes
    /// from the expected type, and the body checks against it. `with`-bound
    /// names (`combine … with start 0 using start plus it`) are in scope for
    /// the lambda body (§7.15's lambda note).
    fn check_lambda_expected(
        &mut self,
        params: &[Name],
        body: &ast::LambdaBody,
        expected: Option<&Type>,
        span: Span,
    ) -> TypedExpr {
        self.push_scope();
        let mut param_tys: Vec<Type> = Vec::new();
        let expected_params: &[Type] = match expected {
            Some(Type::Function(ps, _)) => ps,
            _ => &[],
        };
        for (i, p) in params.iter().enumerate() {
            let pname = p.display();
            // Parameters are inferred: no annotation exists on lambda params
            // (11.1). When the use site knows the parameter type (a combinator
            // over a typed list), the body checks against it; otherwise the
            // body checks with unknowns (M0's inference scope).
            let ty = expected_params.get(i).cloned().unwrap_or(Type::Error);
            self.declare(&pname, ty.clone(), false, p.span);
            param_tys.push(ty);
        }
        let (ret, checked_body) = match body {
            ast::LambdaBody::Block(block) => {
                // A block lambda is a function body (11.1): `gives back` is
                // legal inside it and names its answer. Save/restore the
                // enclosing body context around the nested walk.
                let saved_body = self.current_body;
                let saved_returns = self.current_returns.clone();
                self.current_body = Body::Function;
                self.current_returns = None;
                let stmts = self.check_block_stmts(block);
                self.current_body = saved_body;
                self.current_returns = saved_returns;
                // The lambda's return type: the first `gives back`'s value.
                let mut ret = Type::Error;
                for s in &stmts {
                    if let CheckedStmtKind::GiveBack { value } = &s.kind {
                        ret = value.ty.clone();
                        break;
                    }
                }
                // Keep the checked statements for HIR (the side table keyed by
                // the lambda's span — §11.1: the block form is a full function).
                self.lambda_bodies.insert(span, stmts.clone());
                (ret, ast::LambdaBody::Block(ast::Block { stmts: vec![], span: block.span }))
            }
            ast::LambdaBody::Inline(expr) => {
                let inner_expected = expected
                    .and_then(|t| match t {
                        Type::Function(_, r) => Some((**r).clone()),
                        _ => None,
                    })
                    .map(|r| Box::new(r));
                let t = self.check_expr(expr, inner_expected.as_deref());
                (t.ty.clone(), ast::LambdaBody::Inline(Box::new(t.expr)))
            }
        };
        self.pop_scope();
        let rebuilt = Expr::Lambda { params: params.to_vec(), body: checked_body, span };
        TypedExpr { expr: rebuilt, ty: Type::Function(param_tys, Box::new(ret)) }
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
                    )
                    .with_explanation(
                        "A construction builds either a structure (`structure player …`) or a kind's variant (`kind shape … is a circle with radius …`).",
                    )
                    .with_note(format!(
                        "if `{sname}` is a variant, its `kind` must be declared first",
                    )),
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
                            )
                            .with_explanation("A construction sets each field once — a field needs one value."),
                        );
                    }
                }
                TypedExpr { expr: outer.clone(), ty: Type::Struct(sname) }
            }
        }
    }

    /// §7.0.3 greedy resolution of a word-run in expression position.
    fn check_name_expr(&mut self, name: &Name, span: Span, _expected: Option<&Type>) -> TypedExpr {
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
        //      FIRST-CLASS functions (11.1): a bare name for a function that
        //      takes parameters IS the function value — `using double`,
        //      `make f equal to area` (11.1's own example) never auto-call.
        //      Zero-parameter functions still auto-call (the M0 zero-arg-call
        //      shape; `risky business` used for effect).
        if self.functions.contains_key(&full) {
            let sig = self.functions.get(&full).cloned();
            let takes_params = matches!(sig.as_ref().map(|s| s.params.len()), Some(n) if n > 0);
            if takes_params {
                let (params, ret) = match sig {
                    Some(s) => (s.params, s.ret),
                    None => (Vec::new(), Type::Error),
                };
                let fty = Type::Function(params, Box::new(ret));
                self.node_types.insert(span, fty.clone());
                return TypedExpr {
                    expr: Expr::Name { name: name.clone(), span },
                    ty: fty,
                };
            }
            let call = CallExpr {
                callee: Name { words: vec![full.clone()], span },
                first: None,
                preps: Vec::new(),
                and_args: Vec::new(),
                with_args: Vec::new(),
                using_arg: None,
                where_expr: None,
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
                    using_arg: None,
                    where_expr: None,
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
        // §7.0.3 greedy callee resolution, longest reading first: the whole
        // run as one callable (`json text from data`), then a known-function
        // prefix split (`say square root of 16` parses as callee `say square
        // root` → say(square root(16))).
        let call = self.rejoin_split_builtin(call);
        let resolved = self.resolve_callee(&call);
        self.check_resolved_call(&resolved, expected)
    }

    /// Multi-word builtins containing a type word (`json text`) lex as a name
    /// run split by the type keyword, so the parser hands sema
    /// `json(text(from data))`. When the callee plus the inner call's callee
    /// form a known function name and the inner call is purely prepositional,
    /// rejoin them — the longest-known reading (7.0.3).
    fn rejoin_split_builtin(&self, call: &ast::CallExpr) -> ast::CallExpr {
        let Some(first) = &call.first else { return call.clone() };
        let Expr::Call(inner) = first.expr.as_ref() else { return call.clone() };
        if !inner.first.is_none() || !inner.and_args.is_empty() || !inner.with_args.is_empty()
            || inner.using_arg.is_some() || inner.where_expr.is_some() || inner.preps.len() != 1
            || inner.callee.words.len() != 1 || call.callee.words.len() != 1
        {
            return call.clone();
        }
        let joined = format!("{} {}", call.callee.display(), inner.callee.display());
        if !self.functions.contains_key(&joined) {
            return call.clone();
        }
        let mut out = call.clone();
        out.callee = Name { words: vec![joined], span: call.callee.span.to(inner.callee.span) };
        out.first = inner.first.clone();
        out.preps = inner.preps.clone();
        out.span = call.span;
        out
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
        // Greedy longest match (7.0.3): the whole run wins before any split —
        // `json text from data` is the `json text` builtin, never
        // `json(text from data)`.
        if words.len() > 1 && self.functions.contains_key(&call.callee.display()) {
            return call.clone();
        }
        if words.len() > 1 {
            for split in (1..words.len()).rev() {
                let head = words[..split].join(" ");
                if self.functions.contains_key(&head) {
                    // Same invariant as the name-split path: the synthetic
                    // argument (and the inner call it wraps) takes the tail
                    // words' own span, distinct from the outer call's.
                    // The `using`/`where` suffixes belong to the OUTER call
                    // (they are R-3's call suffixes of the whole sentence:
                    // `map xs using area` = map(xs, λarea), never xs(λarea)).
                    let tail_span = self.tail_words_span(call.callee.span, words, split);
                    let tail_name = Name { words: words[split..].to_vec(), span: tail_span };
                    let tail_arg = if call.first.is_none()
                        && call.preps.is_empty()
                    {
                        // A tail with no arguments of its own (`map xs`, `say
                        // result`) is a plain NAME — the tail words ARE the
                        // argument value (S-14: a non-function callee with no
                        // prepositional args is a read/name, never a zero-arg
                        // call). Only a tail that carries arguments stays a
                        // call (`say square root of 16`).
                        Expr::Name { name: tail_name.clone(), span: tail_span }
                    } else {
                        // The tail continues its own flowcall: its [additive]
                        // (outer `first`) and { prep } args. The `and`/`with`
                        // suffixes belong to the OUTER flowcall —
                        // `split line and ","` is split(line, ",") because
                        // `and` continues the flowcall whose name the parser
                        // read as one run (G-22's second half).
                        Expr::Call(Box::new(ast::CallExpr {
                            callee: tail_name.clone(),
                            first: call.first.clone(),
                            preps: call.preps.clone(),
                            and_args: Vec::new(),
                            with_args: Vec::new(),
                            using_arg: None,
                            where_expr: None,
                            span: tail_span,
                        }))
                    };
                    // Arity check: when the tail is a known function whose
                    // parameter count exactly matches the arguments the outer
                    // call carries beyond the tail read (`say add 2 and 3` —
                    // tail `add` takes 2, the call's `2 and 3` supplies both),
                    // the and/with args belong to the TAIL call, not the head.
                    // The tail already got the `first`/preps; only and/with
                    // can still be misrouted, so transfer them.
                    let tail_str = tail_name.display();
                    let tail_is_fn = self.functions.contains_key(&tail_str);
                    // G-25: when the tail is a PLAIN NAME (no function), the
                    // preps the lexer's one-run read gave it are the HEAD's
                    // second positional argument, not reads on the tail —
                    // `write file c at t` is write_file(c, t), never
                    // write_file(c.at(t)). The head's arity decides: with
                    // room for the tail read plus every prep, they stay
                    // positional; a real tail call keeps its own preps
                    // (the branch above).
                    if !tail_is_fn && !call.preps.is_empty() {
                        if let Some(hsig) = self.functions.get(&head) {
                            let positional = 1 + call.preps.len();
                            let extra = call.and_args.len() + call.with_args.len();
                            if hsig.params.len() == positional
                                && extra == 0
                                && call.using_arg.is_none()
                                && call.where_expr.is_none()
                            {
                                let mut args: Vec<ast::Arg> = Vec::new();
                                if let Some(f) = &call.first {
                                    args.push((**f).clone());
                                }
                                args.push(ast::Arg {
                                    expr: Box::new(Expr::Name { name: tail_name.clone(), span: tail_span }),
                                    span: tail_span,
                                });
                                for (_, a) in &call.preps {
                                    args.push(a.clone());
                                }
                                let mut first = None;
                                let mut and_args = Vec::new();
                                if args.len() > 1 {
                                    first = Some(Box::new(args.remove(0)));
                                    and_args = args.drain(..).collect();
                                }
                                return ast::CallExpr {
                                    callee: Name { words: vec![head], span: call.callee.span },
                                    first,
                                    preps: Vec::new(),
                                    and_args,
                                    with_args: Vec::new(),
                                    using_arg: None,
                                    where_expr: None,
                                    span: call.span,
                                };
                            }
                        }
                    }
                    if let Some(tsig) = self.functions.get(&tail_str) {
                        let tail_fed = call.first.as_ref().map_or(0, |_| 1)
                            + call.preps.len();
                        if tsig.params.len() > tail_fed
                            && tsig.params.len() == tail_fed + call.and_args.len() + call.with_args.len()
                            && !(call.and_args.is_empty() && call.with_args.is_empty())
                        {
                            return ast::CallExpr {
                                callee: Name { words: vec![head], span: call.callee.span },
                                first: Some(Box::new(ast::Arg {
                                    expr: Box::new(Expr::Call(Box::new(ast::CallExpr {
                                        callee: tail_name.clone(),
                                        first: call.first.clone(),
                                        preps: call.preps.clone(),
                                        and_args: call.and_args.clone(),
                                        with_args: call.with_args.clone(),
                                        using_arg: None,
                                        where_expr: None,
                                        span: tail_span,
                                    }))),
                                    span: tail_span,
                                })),
                                preps: Vec::new(),
                                and_args: Vec::new(),
                                with_args: Vec::new(),
                                using_arg: call.using_arg.clone(),
                                where_expr: call.where_expr.clone(),
                                span: call.span,
                            };
                        }
                    }
                    return ast::CallExpr {
                        callee: Name { words: vec![head], span: call.callee.span },
                        first: Some(Box::new(ast::Arg {
                            expr: Box::new(tail_arg),
                            span: tail_span,
                        })),
                        preps: Vec::new(),
                        and_args: call.and_args.clone(),
                        with_args: call.with_args.clone(),
                        using_arg: call.using_arg.clone(),
                        where_expr: call.where_expr.clone(),
                        span: call.span,
                    };
                }
            }
        }
        call.clone()
    }

    /// R-3's one desugaring rule: `where <orexpr>` on a call is the inline
    /// lambda `taking it giving back <expr>` (00 §7.9). Applied before arity
    /// checking so the `using` path sees an ordinary lambda argument.
    fn desugar_where(&mut self, call: &ast::CallExpr) -> ast::CallExpr {
        let Some(where_expr) = &call.where_expr else {
            return call.clone();
        };
        let mut out = call.clone();
        // `it` as the single parameter (11.1's implicit single parameter).
        let it = Name { words: vec!["it".to_string()], span: expr_span(where_expr) };
        out.using_arg = Some(Box::new(Expr::Lambda {
            params: vec![it],
            body: ast::LambdaBody::Inline(Box::new((**where_expr).clone())),
            span: expr_span(where_expr),
        }));
        out.where_expr = None;
        out
    }

    /// 11.2's implicit-parameter rule: a `using` argument written as a bare
    /// comparison (`using it plus 5`, `using it is at least 80`) is the
    /// inline lambda `taking it giving back <expr>`. The `it` in scope makes
    /// the element name; an explicit `taking …` lambda is untouched. A bare
    /// name (`using double`) stays a function-value read. For `combine`, the
    /// first `with`-bound name is the accumulator: it becomes the lambda's
    /// LEADING formal (`using start plus it` = `taking start and it giving
    /// back start plus it`, §11.2), not a captured constant.
    fn desugar_using(&mut self, call: &ast::CallExpr) -> ast::CallExpr {
        let Some(lam) = &call.using_arg else {
            return call.clone();
        };
        let callee = call.callee.display();
        let is_combine = callee == "combine";
        let is_bare_it_expr = match &**lam {
            Expr::Lambda { .. } => false,
            // Any non-lambda, non-bare-name expression is an implicit-`it`
            // body (`it plus 5`); a bare `Name` may be a function value.
            Expr::Name { .. } => false,
            _ => true,
        };
        if !is_bare_it_expr {
            return call.clone();
        }
        let mut out = call.clone();
        let span = expr_span(lam);
        let mut params = vec![Name { words: vec!["it".to_string()], span }];
        if is_combine {
            // The accumulator's name is the first `with` label (`with start
            // 0`); it leads the formals so the fold can rebind it each step.
            if let Some((acc, _)) = call.with_args.first() {
                params.insert(0, acc.clone());
            }
        }
        out.using_arg = Some(Box::new(Expr::Lambda {
            params,
            body: ast::LambdaBody::Inline(Box::new((**lam).clone())),
            span,
        }));
        out
    }

    fn check_resolved_call(&mut self, call: &ast::CallExpr, _expected: Option<&Type>) -> TypedExpr {
        // M1 call suffixes (R-3): `where <orexpr>` desugars to the inline
        // lambda `taking it giving back <expr>` — one desugaring rule, checked
        // as the lambda it means. It must exist before arity checking adds
        // its argument.
        let call = self.desugar_where(call);
        let call = self.desugar_using(&call);        // The `using` lambda is checked AFTER the leading list argument (the
        // combinator callees below), so the element type can seed the lambda's
        // parameter type (11.2: the combinator gives the lambda its input).
        // `with`-suffix names (`combine … with start 0 using start plus it`)
        // are in scope for the lambda body per §7.15's lambda note.
        let mut extra_args: Vec<Expr> = Vec::new();
        let mut checked_extra: Vec<TypedExpr> = Vec::new();
        let mut with_scope: Vec<(String, Type)> = Vec::new();
        if !call.with_args.is_empty() {
            self.push_scope();
            for (wname, warg) in &call.with_args {
                let wt = self.check_expr(&warg.expr, None);
                self.declare(&wname.display(), wt.ty.clone(), false, wname.span);
                with_scope.push((wname.display(), wt.ty.clone()));
            }
        }
        if let Some(lam) = &call.using_arg {
            extra_args.push((**lam).clone());
        }
        let callee = call.callee.display();
        // LOM provenance: function-call events key on the callee span (26.5).
        self.callee_spans.insert(call.span, call.callee.span);
        // Arguments in call order (the `using` lambda is the last argument —
        // R-3's fixed suffix order).
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
        args.extend(extra_args.iter());
        let sig = self.functions.get(&callee).cloned();
        match sig {
            None => {
                // Function-value application (11.1): a callee *binding* that
                // holds a function applies its arguments — `make f equal to
                // double` then `f of 21`, `f 5`, `f of f of 3`. Rewrite to the
                // proven runtime shape `call <callee> <args…>` (MIR's CallClosure
                // path) — one dispatcher, no second application semantics.
                // Field/index reads fall through below: their callee is a
                // structure/indexed binding, not a function binding.
                if matches!(
                    self.lookup(&callee),
                    Some(Binding { ty: Type::Function(_, _), .. })
                ) {
                    let mut synthetic = CallExpr {
                        callee: Name { words: vec!["call".into()], span: call.callee.span },
                        first: Some(Box::new(ast::Arg {
                            expr: Box::new(ast::Expr::Name { name: call.callee.clone(), span: call.callee.span }),
                            span: call.callee.span,
                        })),
                        preps: Vec::new(),
                        and_args: Vec::new(),
                        with_args: Vec::new(),
                        using_arg: call.using_arg.clone(),
                        where_expr: call.where_expr.clone(),
                        span: call.span,
                    };
                    if let Some(f) = &call.first {
                        synthetic.and_args.push((**f).clone());
                    }
                    for (_, a) in &call.preps {
                        synthetic.and_args.push(a.clone());
                    }
                    synthetic.and_args.extend(call.and_args.iter().cloned());
                    let out = self.check_call(&synthetic, None);
                    if !with_scope.is_empty() {
                        self.pop_scope();
                    }
                    return out;
                }
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
                            let rebuilt = self.rebuild_call(&call, vec![base.clone()]);
                            self.read_spans.insert(call.span);
                            let out = self.field_read(&callee, base, rebuilt, arg_span);
                            if !with_scope.is_empty() {
                                self.pop_scope();
                            }
                            return out;
                        }
                        Prep::At => {
                            let base = self.check_name_expr(&call.callee, call.callee.span, None);
                            let idx_expected = match &base.ty {
                                Type::List(_) => Some(Type::Number),
                                Type::Map(k, _) => Some((**k).clone()),
                                _ => None,
                            };
                            let index = self.check_expr(arg_expr, idx_expected.as_ref());
                            let rebuilt = self.rebuild_call(&call, vec![index.clone()]);
                            self.read_spans.insert(call.span);
                            let out = self.index_read(base, index, rebuilt, arg_span);
                            if !with_scope.is_empty() {
                                self.pop_scope();
                            }
                            return out;
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
                        )
                        .with_explanation("Only functions take arguments — compute with the value instead (for example, `total plus 3`), or call a function that exists."),
                    );
                } else {
                    let d = self.unknown_name(&callee, call.callee.span);
                    self.diags.push(d);
                }
                for a in &args {
                    self.check_expr(a, None);
                }
                let out = TypedExpr { expr: self.rebuild_call(&call, Vec::new()), ty: Type::Error };
                if !with_scope.is_empty() {
                    self.pop_scope();
                }
                return out;
            }
            Some(sig) => {
                let n_pre = args.len() - extra_args.len();
                let mut checked: Vec<TypedExpr> =
                    args.iter().take(n_pre).map(|a| self.check_expr(a, None)).collect();
                // The `using` lambda: now that the leading arguments are
                // checked, derive its expected parameter type from the
                // combinator's list element (`map a list of 1, 2 using it plus
                // 5` checks `it` as a number — 11.2). `map`/`keep` take the
                // function as their second argument; `combine` as its third.
                if !extra_args.is_empty() {
                    // `map`/`keep`: one formal (the element). `combine`: two
                    // formals (accumulator, element) — 11.2's fold shape.
                    let elem: Option<Type> = match callee.as_str() {
                        "map" | "keep" | "combine" => checked.first().and_then(|c| match &c.ty {
                            Type::List(e) => Some((**e).clone()),
                            _ => None,
                        }),
                        _ => None,
                    };
                    let acc: Option<Type> = match callee.as_str() {
                        "combine" => checked.get(1).map(|c| c.ty.clone()),
                        _ => None,
                    };
                    let ret_expected: Option<Type> = match callee.as_str() {
                        "keep" => Some(Type::Boolean),
                        "combine" => acc.clone(),
                        _ => None,
                    };
                    for lam in &extra_args {
                        let expected_fn = elem.as_ref().map(|e| {
                            let mut ps = Vec::new();
                            if let Some(a) = &acc {
                                ps.push(a.clone());
                            }
                            ps.push(e.clone());
                            Type::Function(ps, Box::new(ret_expected.clone().unwrap_or(Type::Error)))
                        });
                        let lt = self.check_lambda_expected_lambda(lam, expected_fn.as_ref());
                        if !matches!(lt.ty, Type::Function(_, _) | Type::Error) {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0366",
                                    format!("`using` needs a function, but this is {}.", lt.ty.display()),
                                    lt.span(),
                                )
                                .with_explanation("A `using` argument is applied to each element — write a lambda: `using it plus 5` or `taking n giving back n times 2`."),
                            );
                        }
                        checked_extra.push(lt);
                    }
                }
                checked.extend(checked_extra.iter().cloned());
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
                                        )
                                        .with_explanation("`first of` answers a list's first item — `nothing` when the list is empty (D-34)."),
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
                                        )
                                        .with_explanation("`size of` counts what has items: a list, a map, or a piece of text."),
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
                                .with_fix("use math")
                                .with_fix_why("the name belongs to the `math` module, and a module's functions are only in scope once the file uses it"),
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
                    // G-27: `bigger of a and b` / `smaller of a and b` — the
                    // §7.8 exemplars, in the math module, two numbers in,
                    // the larger/smaller one out.
                    "bigger" | "smaller" => {
                        if !self.is_used("math") {
                            self.diags.push(
                                Diagnostic::error(
                                    "E0353",
                                    format!("`{callee}` is in the `math` module — write `use math` at the top of the file."),
                                    call.callee.span,
                                )
                                .with_fix("use math")
                                .with_fix_why("the name belongs to the `math` module, and a module's functions are only in scope once the file uses it"),
                            );
                        }
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Number, &format!("`{callee}`"), "a number");
                            self.require_type(&checked[1], &Type::Number, &format!("`{callee}`"), "a number");
                        }
                        (Type::Number, false)
                    }
                    // ----- M1 combinators (§11.2): stdlib over lists + a
                    // function argument. `with start` is combine's labeled
                    // start value; the lambda is the last argument. -----
                    "map" => {
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        }
                        if let Some(f) = checked.get(1) {
                            self.require_function_of_1(f, &callee);
                        }
                        match checked.first().map(|c| c.ty.clone()) {
                            Some(Type::List(e)) => (Type::List(Box::new(self.apply_result(&e, checked.get(1)))), false),
                            Some(Type::Error) => (Type::Error, false),
                            Some(other) => {
                                self.diags.push(self.combinator_needs_list(&callee, &other, call.callee.span));
                                (Type::Error, false)
                            }
                            None => (Type::Error, false),
                        }
                    }
                    "keep" => {
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        }
                        if let Some(f) = checked.get(1) {
                            self.require_predicate(f, &callee);
                        }
                        match checked.first().map(|c| c.ty.clone()) {
                            Some(t) if matches!(t, Type::List(_) | Type::Error) => (t, false),
                            Some(other) => {
                                self.diags.push(self.combinator_needs_list(&callee, &other, call.callee.span));
                                (Type::Error, false)
                            }
                            None => (Type::Error, false),
                        }
                    }
                    "combine" => {
                        // `combine items with start 0 using start plus it`:
                        // 3 arguments (list, start, function). The function
                        // takes (accumulator, element) and the result type is
                        // the start value's type (11.2's fold).
                        if n != 3 {
                            self.diags.push(self.bad_arity(&callee, 3, n, call.callee.span));
                        }
                        let start_ty = checked.get(1).map(|c| c.ty.clone()).unwrap_or(Type::Error);
                        if let Some(f) = checked.get(2) {
                            self.require_function_of_2(f, &callee, &start_ty);
                        }
                        match checked.first().map(|c| c.ty.clone()) {
                            Some(Type::List(_)) | Some(Type::Error) | None => (start_ty, false),
                            Some(other) => {
                                self.diags.push(self.combinator_needs_list(&callee, &other, call.callee.span));
                                (Type::Error, false)
                            }
                        }
                    }
                    // ----- text module surface at M1 (docs/07: split/join/
                    // trim/contains are the student text-processing set).
                    // All spellings fit the frozen call grammar: `split text
                    // and separator` (R-1's `and`-separated args). -----
                    "split" => {
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`split … and …`", "the text");
                            self.require_type(&checked[1], &Type::Text, "`split … and …`", "the separator");
                        }
                        (Type::List(Box::new(Type::Text)), false)
                    }
                    "contains" => {
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`contains`", "the text");
                            self.require_type(&checked[1], &Type::Text, "`contains`", "the piece to look for");
                        }
                        (Type::Boolean, false)
                    }
                    "sort" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        }
                        match checked.first().map(|c| c.ty.clone()) {
                            Some(t) if matches!(t, Type::List(_) | Type::Error) => (t, false),
                            Some(other) => {
                                self.diags.push(self.combinator_needs_list(&callee, &other, call.callee.span));
                                (Type::Error, false)
                            }
                            None => (Type::Error, false),
                        }
                    }
                    // ----- files module (§19.1, roadmap M1). Spellings fit
                    // the frozen grammar and §7.13's own example: `open file
                    // at path` (a multi-word callee + the `at` preposition),
                    // `write text at path` (callee `write file`). Every file
                    // operation can fail (§13.1; at M1 the failure value is
                    // the message text, S-10). -----
                    "open file" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`open file at …`", "the file path");
                        }
                        (Type::Text, true)
                    }
                    "write file" => {
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`write file at path`", "the text to write");
                            self.require_type(&checked[1], &Type::Text, "`write file at path`", "the file path");
                        }
                        (Type::Text, true)
                    }
                    "append file" => {
                        if n != 2 {
                            self.diags.push(self.bad_arity(&callee, 2, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`append file at path`", "the text to append");
                            self.require_type(&checked[1], &Type::Text, "`append file at path`", "the file path");
                        }
                        (Type::Text, true)
                    }
                    "delete file" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`delete file at …`", "the file path");
                        }
                        (Type::Text, true)
                    }
                    "file exists" | "file size" => {
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, &format!("`{callee} at …`"), "the file path");
                        }
                        let ty = if callee == "file exists" { Type::Boolean } else { Type::Number };
                        (ty, false)
                    }
                    // ----- json module (§19.1). Spellings mirror D-39's
                    // conversion pattern and fit the frozen call grammar:
                    // `json from text` parses (can fail — bad input);
                    // `json text from value` formats (cannot fail). The M1
                    // JSON value surface is a text→text map (the teaching
                    // subset; richer shapes come with the M2+ batteries
                    // json module, docs/07 open question). -----
                    "json" => {
                        // `json from <text>` — parse JSON text (can fail).
                        // Result: a map from text to text (student values).
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        } else {
                            self.require_type(&checked[0], &Type::Text, "`json from …`", "the JSON text");
                        }
                        (Type::Map(Box::new(Type::Text), Box::new(Type::Text)), true)
                    }
                    "json text" => {
                        // `json text from <value>` — format a value (S-9's
                        // rules; maps format as JSON objects).
                        if n != 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        }
                        (Type::Text, false)
                    }
                    "call" => {
                        // `call <closure> [with args…]` — apply a closure
                        // value (11.1). The closure's function type fixes the
                        // result; an unknown operand keeps Error.
                        if n < 1 {
                            self.diags.push(self.bad_arity(&callee, 1, n, call.callee.span));
                        }
                        let ty = match checked.first().map(|c| c.ty.clone()) {
                            Some(Type::Function(_, ret)) => *ret,
                            _ => Type::Error,
                        };
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
                let rebuilt = self.rebuild_call(&call, checked);
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
        // The `using` lambda is the trailing checked argument (R-3's fixed
        // suffix order) — swap in its checked form so HIR lowers the closure
        // value, not the raw source text.
        if out.using_arg.is_some() {
            if let Some(t) = it.next() {
                out.using_arg = Some(Box::new(t.expr));
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
                            format!("`{sname}` has no field called `{field}`."),
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
            Type::Text => {
                // §7.7: `greeting at 2` — Unicode code point by position
                // (0-based, like list indexing). Produces a 1-character text.
                if !matches!(index.ty, Type::Number | Type::Error) {
                    self.diags.push(
                        Diagnostic::error(
                            "E0342",
                            format!("A text index must be a number, but this is {}.", index.ty.display()),
                            span,
                        ),
                    );
                }
                Type::Text
            }
            Type::Error => Type::Error,
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0342",
                        format!("`at` needs a list, a map, or text, but this is {}.", other.display()),
                        span,
                    )
                    .with_explanation("`at` reads one item from a list, one entry from a map, or one character from text."),
                );
                Type::Error
            }
        };
        TypedExpr { expr: rebuilt, ty }
    }

    /// `binding origin` (M2 diagnostics): where a name was made — the
    /// line a secondary label quotes so the error explains the value's
    /// provenance, not just its current type.
    fn binding_label(&self, name: &str, what: &str) -> Option<(Span, String)> {
        let b = self.scopes.iter().rev().find_map(|s| s.bindings.get(name))?;
        Some((b.span, format!("{what} (made at line {})", lagom_diagnostics::SourceFile::new("", self.src).line_col(b.span.start).0)))
    }

    fn unhandled_failure(&self, callee: &str, span: Span) -> Diagnostic {
        // M2: the fix quotes the student's actual call — the root cause is
        // *this unhandled call*, so the fix wraps exactly it, not a skeleton
        // the student has to transplant (a transplant invites a fix-after-fix
        // cycle when the call has arguments or `and` arguments).
        let call = self
            .src
            .get(span.start..span.end.min(self.src.len()))
            .unwrap_or("")
            .trim();
        Diagnostic::error(
            "E0302",
            format!("`{callee}` can fail, so this call must be wrapped in `attempt`."),
            span,
        )
        .with_explanation("A function that can fail must be handled: wrap the call in `attempt … if it fails then … otherwise …`, bind the problem with `as`, or pass it on with `and pass the problem on`.")
        .with_fix(format!("attempt {call} if it fails then\n    say problem\notherwise\n    say result"))
        .with_fix_why(format!("the `attempt` turns the error into a value your program handles on the `if it fails` branch — the failure from `{callee}` can no longer escape unhandled"))
    }

    fn bad_arity(&self, callee: &str, want: usize, got: usize, span: Span) -> Diagnostic {
        Diagnostic::error(
            "E0354",
            format!("`{callee}` needs {want} value{}, but this call gives {got}.", if want == 1 { "" } else { "s" }),
            span,
        )
    }

    /// The `using` argument of `map`/`combine` must be a one-parameter
    /// function (11.2); `it` lambdas and `where` sugar produce exactly that.
    fn require_function_of_1(&mut self, f: &TypedExpr, callee: &str) {
        match &f.ty {
            Type::Function(params, _) if params.len() == 1 => {}
            Type::Function(params, _) => {
                self.diags.push(                                Diagnostic::error(
                                    "E0369",
                                    format!("`{callee}` applies a function taking one value, but this function takes {}.", params.len()),
                                    f.span(),
                                )
                                .with_explanation("The combinator calls its function once per element — the function takes exactly the element."),
                );
            }
            Type::Error => {}
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0366",
                        format!("`{callee}` needs a function, but this is {}.", other.display()),
                        f.span(),
                    )
                    .with_explanation("Write the per-element step as a lambda: `using it plus 5` or `taking n giving back n times 2`."),
                );
            }
        }
    }

    /// `keep`'s `where`/`using` argument must produce a boolean.
    fn require_predicate(&mut self, f: &TypedExpr, callee: &str) {
        match &f.ty {
            Type::Function(_, ret) if matches!(**ret, Type::Boolean | Type::Error) => {}
            Type::Function(_, ret) => {
                self.diags.push(
                    Diagnostic::error(
                        "E0369",
                        format!("`{callee}` keeps elements whose function answers true, but this function gives back {}.", ret.display()),
                        f.span(),
                    ),
                );
            }
            Type::Error => {}
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0366",
                        format!("`{callee}` needs a function, but this is {}.", other.display()),
                        f.span(),
                    )
                    .with_explanation("Write the test as a `where` clause: `keep scores where it is at least 80`."),
                );
            }
        }
    }

    /// `combine`'s `using` argument takes (accumulator, element) and answers
    /// the accumulator's type (11.2's fold shape).
    fn require_function_of_2(&mut self, f: &TypedExpr, callee: &str, acc_ty: &Type) {
        match &f.ty {
            Type::Function(params, ret) if params.len() == 2 => {
                if !matches!(**ret, Type::Error) && !matches!(acc_ty, Type::Error) && *ret.clone() != *acc_ty {
                    self.diags.push(
                        Diagnostic::error(
                            "E0369",
                            format!(
                                "`{callee}` gives back the accumulated value, but this function answers {} while the start value is {}.",
                                ret.display(),
                                acc_ty.display()
                            ),
                            f.span(),
                        ),
                    );
                }
            }
            Type::Function(params, _) => {
                self.diags.push(
                    Diagnostic::error(
                        "E0369",
                        format!(
                            "`{callee}` applies a function taking the accumulated value and each element (two values), but this function takes {}.",
                            params.len()
                        ),
                        f.span(),
                    ),
                );
            }
            Type::Error => {}
            other => {
                self.diags.push(
                    Diagnostic::error(
                        "E0366",
                        format!("`{callee}` needs a function, but this is {}.", other.display()),
                        f.span(),
                    )
                    .with_explanation("Write the fold step as a lambda: `using start plus it`."),
                );
            }
        }
    }

    /// The element type `map` produces: the function's return type when
    /// known, otherwise the input element type (inference fallback).
    fn apply_result(&self, elem: &Type, f: Option<&TypedExpr>) -> Type {
        match f.map(|t| &t.ty) {
            Some(Type::Function(_, ret)) if !matches!(**ret, Type::Error) => (**ret).clone(),
            _ => elem.clone(),
        }
    }

    fn combinator_needs_list(&self, callee: &str, got: &Type, span: Span) -> Diagnostic {
        Diagnostic::error(
            "E0370",
            format!("`{callee}` works on a list, but this is {}.", got.display()),
            span,
        )
        .with_explanation("The combinators walk a list one element at a time.")
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
                (a, b) => {
                    unify_numeric(a, b).is_some()
                        || a == b
                        // R-6: options compare transparently — `first of xs
                        // is equal to 7` reads the contained value, the same
                        // value reading 8.5 gives `say` of an option.
                        || option_holds(a, b)
                        || option_holds(b, a)
                }
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
            return TypedExpr { expr: e_binary(op, &lt.expr, &rt.expr, span), ty: Type::Boolean };
        }
        // Ordering: numbers only (no truthiness — 8.3).
        if matches!(op, BinOp::Greater | BinOp::Less | BinOp::AtLeast | BinOp::AtMost) {
            let lt = self.check_expr(left, None);
            let rt = self.check_expr(right, None);
            let numeric = (lt.ty.is_numeric() || matches!(lt.ty, Type::Error))
                && (rt.ty.is_numeric() || matches!(rt.ty, Type::Error));
            if !numeric {
                let mut d = Diagnostic::error(
                    "E0356",
                    format!("`is {}` needs two numbers, but got {} and {}.", op_word(op), lt.ty.display(), rt.ty.display()),
                    span,
                )
                .with_explanation("Ordering (bigger/smaller) only makes sense for numbers.");
                // M2 provenance: point at where the non-numeric value was
                // made, so the student sees the value's origin, not just its
                // current type at the comparison.
                for side in [&lt, &rt] {
                    if let Expr::Name { name, .. } = &side.expr {
                        if let Some((sp, label)) = self.binding_label(&name.display(), "the value was made here") {
                            d = d.with_label(sp, label);
                        }
                    }
                }
                if matches!(rt.ty, Type::Text) || matches!(lt.ty, Type::Text) {
                    // The right side is usually what the student meant to be
                    // text — quote it as the value the literal rewrite keeps.
                    let right_text = match &right {
                        Expr::Text { value, .. } => format!("\"{value}\""),
                        _ => String::from("…"),
                    };
                    d = d.with_fix(format!("is equal to {right_text}"));
                    d = d.with_fix_why("text has no bigger/smaller order in Lagom — equality is the comparison text supports, and this keeps the text value the program already has");
                }
                self.diags.push(d);
            }
            return TypedExpr { expr: e_binary(op, &lt.expr, &rt.expr, span), ty: Type::Boolean };
        }
        // Boolean operators: strictly boolean (S-4, doc 03's open question
        // resolved for M0).
        // Boolean operators: strictly boolean (S-4, doc 03's open question
        // resolved for M0).
        // G-22 (docs/14): an `and` whose LEFT side resolves to a call with
        // exactly one argument so far (`split line and ","`) is that call's
        // NEXT POSITIONAL ARGUMENT, not a boolean operator — the grammar's
        // own exemplars spell two-argument callees exactly this way (00
        // §7.15: `split text and separator`; the parser reads `split line`
        // as one name run, and the rebuilt call surfaces here). The frozen
        // teaching rule already forbids a call as a boolean operand (7.9:
        // boolean arguments need parentheses or the `where` suffix), so the
        // reading is total: `and` after a one-argument call is always an
        // argument separator, never a conjunction.
        if op == BinOp::And {
            // Raw-tree fast path (G-22 completion): when the LEFT side is a
            // bare name run that is NOT a local binding but IS a known
            // callable prefix (`split line` — `split` registered, `line` the
            // argument), the parser's andexpr ladder hands us `split`-and-…
            // and the call's argument list is still OPEN: complete it into
            // one call BEFORE any eager sub-checking (checking `split line`
            // first would fire a premature arity error — `split` needs 2).
            // The existing checked-form arm below still handles calls that
            // arrive already restructured (`say contains "hello" and
            // "ell"` after callee resolution).
            if let Expr::Name { name, .. } = left {
                let full = name.display();
                if self.lookup(&full).is_none() {
                    for split in (1..name.words.len()).rev() {
                        let head = name.words[..split].join(" ");
                        if self.functions.contains_key(&head) {
                            // The remaining words are the call's first
                            // argument (a plain name read or nested flow).
                            let tail_span = self.tail_words_span(span, &name.words, split);
                            let first_arg = ast::Arg {
                                expr: Box::new(Expr::Name {
                                    name: Name { words: name.words[split..].to_vec(), span: tail_span },
                                    span: tail_span,
                                }),
                                span: tail_span,
                            };
                            let mut call = ast::CallExpr {
                                callee: Name { words: vec![head], span: name.span },
                                first: Some(Box::new(first_arg)),
                                preps: Vec::new(),
                                and_args: Vec::new(),
                                with_args: Vec::new(),
                                using_arg: None,
                                where_expr: None,
                                span: name.span,
                            };
                            // Flatten the right side's and-chain into the
                            // argument list (the parser left-associates).
                            let mut cursor: &Expr = right;
                            let mut hoisted_using: Option<Box<Expr>> = None;
                            let mut hoisted_where: Option<Box<Expr>> = None;
                            loop {
                                match cursor {
                                    Expr::Binary { op: BinOp::And, right: r, .. } => {
                                        let rsp = expr_span(r);
                                        call.and_args.push(ast::Arg { expr: Box::new((**r).clone()), span: rsp });
                                        cursor = match r.as_ref() {
                                            Expr::Binary { op: BinOp::And, right: r2, .. } => r2.as_ref(),
                                            _ => break,
                                        };
                                    }
                                    other => {
                                        // The parser closes the final and-arg
                                        // with the sentence's `using`/`where`
                                        // suffix; per R-3 it belongs to the
                                        // outer call — strip and hoist it.
                                        if let Some((stripped, using, whr)) = and_arg_with_suffix(other) {
                                            let osp = expr_span(other);
                                            call.and_args.push(ast::Arg { expr: Box::new(stripped), span: osp });
                                            hoisted_using = using;
                                            hoisted_where = whr;
                                        } else {
                                            let osp = expr_span(other);
                                            call.and_args.push(ast::Arg { expr: Box::new(other.clone()), span: osp });
                                        }
                                        break;
                                    }
                                }
                            }
                            call.and_args.reverse();
                            call.using_arg = hoisted_using.or(call.using_arg.take());
                            call.where_expr = hoisted_where.or(call.where_expr.take());
                            call.span = span;
                            return self.check_call(&call, expected);
                        }
                    }
                }
            }
            let lt = self.check_expr(left, None);
            if is_single_arg_call(&lt.expr) {
                let mut call = match &lt.expr {
                    Expr::Call(c) => (**c).clone(),
                    _ => unreachable!(),
                };
                // Flatten the right side's remaining `and` chain into
                // arguments (the parser left-associates, so walk the spine).
                let mut cursor: &Expr = right;
                let mut hoisted_using: Option<Box<Expr>> = None;
                let mut hoisted_where: Option<Box<Expr>> = None;
                loop {
                    match cursor {
                        Expr::Binary { op: BinOp::And, left: l, right: r, .. } => {
                            let rsp = self.check_expr(r, None);
                            let _ = rsp;
                            // Checked below via the rebuilt call; stash raw.
                            cursor = l;
                            let rspan = expr_span(r);
                            call.and_args.push(ast::Arg { expr: Box::new((**r).clone()), span: rspan });
                        }
                        other => {
                            // The parser closes the final and-arg with the
                            // sentence's `using`/`where` suffix; per R-3 it
                            // belongs to the outer call — strip and hoist it.
                            if let Some((stripped, using, whr)) = and_arg_with_suffix(other) {
                                let osp = expr_span(other);
                                call.and_args.push(ast::Arg { expr: Box::new(stripped), span: osp });
                                hoisted_using = using;
                                hoisted_where = whr;
                            } else {
                                let osp = expr_span(other);
                                call.and_args.push(ast::Arg { expr: Box::new(other.clone()), span: osp });
                            }
                            break;
                        }
                    }
                }
                call.and_args.reverse();
                call.using_arg = hoisted_using.or(call.using_arg.take());
                call.where_expr = hoisted_where.or(call.where_expr.take());
                call.span = span;
                return self.check_call(&call, expected);
            }
            let rt = self.check_expr(right, Some(&Type::Boolean));
            self.require_boolean(&lt, "`and`");
            self.require_boolean(&rt, "`or`");
            return TypedExpr { expr: e_binary(op, &lt.expr, &rt.expr, span), ty: Type::Boolean };
        }
        if matches!(op, BinOp::Or) {
            let lt = self.check_expr(left, Some(&Type::Boolean));
            let rt = self.check_expr(right, Some(&Type::Boolean));
            self.require_boolean(&lt, "`or`");
            self.require_boolean(&rt, "`or`");
            return TypedExpr { expr: e_binary(op, &lt.expr, &rt.expr, span), ty: Type::Boolean };
        }
        // Arithmetic: numeric; mixed promotes to decimal (S-1/S-3); `divided
        // by` always promotes (D-10). **Exception (docs/14 G-20):** `plus` on
        // two texts is concatenation (§8.4's normative sentence). Mixing text
        // with a non-text operand stays an error — interpolation is the
        // teaching form for that (7.7).
        let lt = self.check_expr(left, None);
        let rt = self.check_expr(right, expected);
        if op == BinOp::Add && matches!(lt.ty, Type::Text) && matches!(rt.ty, Type::Text) {
            return TypedExpr { expr: e_binary(op, &lt.expr, &rt.expr, span), ty: Type::Text };
        }
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
        TypedExpr { expr: e_binary(op, &lt.expr, &rt.expr, span), ty }
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
            // `nothing` assigns into any option type (8.5: `nothing` is the
            // absent case of `a T or nothing`); `nothing?`-to-`nothing?` is
            // plain equality via the fallthrough.
            (Type::Option(_), Type::NothingLit) => true,
            // `anything` is the top type (8.2/12.1: "called T elsewhere",
            // inferred from use) — every concrete value is assignable into
            // it, and the checker unifies per call site.
            (Type::Anything, _) => true,
            // §12.1 inside containers: `takes a list of anything called items`
            // takes a list of concrete values (the checker unifies per call
            // site; monomorphization happens in MIR).
            (Type::List(w), Type::List(_)) if matches!(**w, Type::Anything) => true,
            (a, b) => a == b,
        };
        if !ok {
            // M2 provenance: when the mismatched value is a binding, name
            // where it was made — the origin of the wrong type is usually
            // the actual root cause, not the annotated use site. The fix is
            // the value or the annotation, stated causally (never a bare
            // "drop the annotation so the error goes away").
            let mut d = Diagnostic::error(
                "E0360",
                format!("{what} needs {}, but got {}.", want.display(), got.ty.display()),
                got.span(),
            )
            .with_fix(format!(
                "use {} here — or, if {} is what you meant, change the annotation to `of type {}`",
                article_value(want.display()),
                got.ty.display(),
                got.ty.display()
            ))
            .with_fix_why(format!(
                "a value has one type — either give it the {} it needs or change the annotation to {}",
                want.display(),
                got.ty.display()
            ));
            if let Expr::Name { name, .. } = &got.expr {
                if let Some((sp, label)) = self.binding_label(&name.display(), "the value was made here") {
                    d = d.with_label(sp, label);
                }
            }
            self.diags.push(d);
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

/// True when `t` is a user-type reference that names neither a declared
/// structure/kind nor another alias — the unknown-target case for aliases.
fn unknown_alias_ref(t: &Type, raw: &[(String, Type)]) -> Option<String> {
    match t {
        Type::Struct(name) => Some(name.clone()),
        Type::Kind(name) => Some(name.clone()),
        _ => None,
    }
    .filter(|name| !raw.iter().any(|(n, _)| n == name))
}

/// The fix text for "use a {T} value here": containers read naturally with
/// the article inside (`a list of numbers`), so the outer article is only
/// added for the words that need it (D-10's reading-first fix text).
fn article_value(display: String) -> String {
    if display.starts_with("a ") || display.starts_with("an ") || display.ends_with("s") {
        display
    } else {
        format!("a {display} value")
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
        | Stmt::Match { span, .. }
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
    block.stmts.iter().any(|s| stmt_gives_back(s))
}

/// One statement definitely returns: a `gives back`, or a structural form whose
/// every path returns. `match` returns when every `when` arm returns — an
/// `otherwise` is not required because exhaustiveness is enforced separately
/// (E0360 fires when cases are missing, 7.12). `if` returns when every branch
/// returns *and* an `otherwise` exists (a missing `otherwise` can fall through).
fn stmt_gives_back(s: &ast::Stmt) -> bool {
    match s {
        ast::Stmt::GiveBack { .. } | ast::Stmt::FailWith { .. } => true,
        // `attempt … if it fails then <block> otherwise <block>` (and the
        // `as name` tail) covers every path when both sides do: the failure
        // path and the success path are exhaustive by construction (13.1),
        // so a `gives back` in the tail counts as a return. A missing
        // `otherwise` can fall through, so it breaks the guarantee.
        ast::Stmt::Attempt { tail: Some(ast::AttemptTail::IfItFails { then_block, otherwise }), .. } => {
            body_gives_back(then_block)
                && otherwise.as_ref().map(body_gives_back).unwrap_or(false)
        }
        ast::Stmt::Attempt { tail: Some(ast::AttemptTail::As { block, otherwise, .. }), .. } => {
            body_gives_back(block)
                && otherwise.as_ref().map(body_gives_back).unwrap_or(false)
        }
        ast::Stmt::Match { arms, otherwise, .. } => {
            !arms.is_empty()
                && arms.iter().all(|(_, b)| body_gives_back(b))
                && otherwise.as_ref().map(body_gives_back).unwrap_or(true)
        }
        ast::Stmt::If { branches, otherwise, .. } => {
            !branches.is_empty()
                && branches.iter().all(|(_, b)| body_gives_back(b))
                && otherwise.as_ref().map(body_gives_back).unwrap_or(false)
        }
        _ => false,
    }
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

/// R-6: does `opt` (possibly an option) hold a value of `v`'s type — the
/// value reading an option comparison takes (`first of xs is equal to 7`).
fn option_holds(opt: &Type, v: &Type) -> bool {
    match opt {
        Type::Option(inner) => matches!(v, Type::Option(_) | Type::NothingLit) || **inner == *v,
        _ => false,
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
/// Candidate names for an unknown-name error, best first: prefix/substring
/// containment (the old rule) plus single-edit distance (one substitution,
/// insertion, deletion, or adjacent transposition) so `scoer` finds `score`.
/// Distance is capped so unrelated short names are never proposed.
fn did_you_mean(name: &str, candidates: &[String]) -> Vec<String> {
    let lower = name.to_lowercase();
    let mut scored: Vec<(usize, &String)> = Vec::new();
    for c in candidates {
        let cl = c.to_lowercase();
        if cl == lower {
            continue; // exact match would not have been an error
        }
        let d = one_edit_distance(&lower, &cl);
        // Containment (`score` in `scores`) only counts for names of at
        // least 3 characters — a 2-letter name sits inside half the
        // dictionary (`it` in `split`) and the suggestion misleads.
        let containment = (cl.contains(&lower) || lower.contains(&cl)) && lower.chars().count() >= 3;
        if containment || d <= 2 {
            scored.push((d, c));
        }
    }
    scored.sort();
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().map(|(_, c)| c.clone()).take(3).collect()
}

/// Optimal-string-alignment distance (Damerau-Levenshtein without adjacent
/// transposition reuse) — enough for single-typo name suggestions, and small
/// enough to compute on the scope's names for every error.
fn one_edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (al, bl) = (a.len(), b.len());
    if al == 0 {
        return bl;
    }
    if bl == 0 {
        return al;
    }
    let inf = al + bl;
    // d[i][j]: distance between a[..i] and b[..j]; rows are 1-indexed with a
    // guard row/column of `inf` so the transposition neighbour reads stay in
    // bounds (the classic OSA formulation).
    let mut d = vec![vec![inf; bl + 2]; al + 2];
    for (i, row) in d.iter_mut().enumerate().skip(1) {
        row[1] = i - 1;
    }
    for (j, cell) in d[1].iter_mut().enumerate().skip(1) {
        *cell = j - 1;
    }
    for i in 1..=al {
        for j in 1..=bl {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i + 1][j + 1] = (d[i][j] + cost)
                .min(d[i + 1][j] + 1)
                .min(d[i][j + 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i + 1][j + 1] = d[i + 1][j + 1].min(d[i - 1][j - 1] + 1);
            }
        }
    }
    d[al + 1][bl + 1]
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
            "function greet\n    takes text called name\n    returns a text\n    gives back \"Hello, {name}!\"\n\nmake who equal to \"bo\"\nsay greet who\n",
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
            "function greet\n    takes text called name\n    returns a text\n    gives back \"Hello, {name}!\"\n\nmake who equal to \"bo\"\nsay \"{greet who}\"\n",
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

    // ----- M1: kinds, match, options, closures, combinators -----

    const SHAPE_PROGRAM: &str = "\
kind shape
    is a circle with radius of type number
    is a blank\n\
\nfunction describe
    takes shape called s
    returns text
    match s
        when a circle with radius r
            gives back \"round\"
        when blank
            gives back \"empty\"
\nmake f equal to a circle with radius 2
say describe f\n";

    #[test]
    fn kind_declares_and_matches() {
        check_ok(SHAPE_PROGRAM);
    }

    #[test]
    fn match_missing_variant_diagnoses() {
        // Dropping the `when blank` arm trips exhaustiveness (7.12).
        let src = "kind shape\n    is a circle with radius of type number\n    is a blank\n\nfunction describe\n    takes shape called s\n    match s\n        when a circle with radius r\n            gives back \"round\"";
        check_err(src, "E0360");
    }

    #[test]
    fn variant_lit_unknown_name_and_missing_field() {
        // Unknown construction name (neither structure nor variant) → the
        // structure diagnostic with the kind teaching note.
        check_err("make x equal to a circle with radius 2", "E0347");
        check_err(
            "kind shape\n    is a circle with radius of type number\nmake x equal to a circle",
            "E0368",
        );
    }

    #[test]
    fn option_types_check() {
        // `text?` and `a text or nothing` are the same type (8.5/R-18);
        // `first of` produces an option; `nothing` compares only to options.
        check_ok(
            "function head\n    takes text? called maybe\n    returns text?\n    gives back maybe\n\nmake m equal to first of a list of \"x\"\nsay m",
        );
        check_err("make bad equal to nothing plus 1", "E0357");
    }

    #[test]
    fn match_option_arms() {
        check_ok(
            "make m equal to first of a list of \"x\"\nmatch m\n    when nothing\n        say \"empty\"\n    when something with value v\n        say v",
        );
        // `something with value` against a non-option is an error (8.5).
        check_err(
            "match 5\n    when something with value v\n        say v\n    otherwise\n        say \"other\"",
            "E0365",
        );
    }

    #[test]
    fn pair_patterns_destructure() {
        // `a pair of x and y` binds the pair's two halves (7.15's `pair`
        // pattern). `a` is a reserved article — binding names avoid it. The
        // sum is computed with `plus` inside a `make` (an ExprStmt would
        // greedily read `say x plus y` as say(x) plus y — R-4's rule).
        check_ok(
            "make p equal to a pair of 1 and 2\nmatch p\n    when a pair of x and y\n        make total equal to x plus y\n        say total",
        );
    }

    #[test]
    fn lambdas_type_and_check() {
        check_ok("make double equal to taking n giving back n times 2\nmake raised equal to map a list of 1, 2 using double");
        check_ok("make raised equal to map a list of 1, 2 using it plus 5");
        check_ok("make passing equal to keep a list of 85, 40 where it is at least 80");
        check_ok("make total equal to combine a list of 1, 2 with start 0 using start plus it");
        // A bare non-function name as the `using` argument → E0366 (§7.15's
        // lambda production reads a comparison as a per-element function; a
        // bare name is the function-value reading, so a non-function name is
        // the error case).
        check_err(
            "make broken thing equal to 5\nmake also broken equal to map a list of 1 using broken thing",
            "E0366",
        );
        // Combinator over a non-list → E0370.
        check_err("make broken equal to map 5 using it plus 1", "E0370");
    }

    #[test]
    fn type_aliases_are_transparent() {
        // 8.4: the alias IS the type — annotations, params, returns, and list
        // element types all accept it, and it needs no runtime presence.
        check_ok(
            "a type called score is a number\nmake s equal to 15 of type score\nsay s",
        );
        check_ok(
            "a type called score is a number\na type called scores is a list of score\nmake xs equal to a list of 1, 2 of type scores\nsay size of xs",
        );
        check_ok(
            "a type called score is a number\n\nfunction total\n    takes score called s\n    returns a score\n    gives back s plus 1\n\nsay total 4",
        );
        // Chain: an alias of an alias, declared in either order (7.13).
        check_ok(
            "a type called tally is a count\na type called count is a number\nmake t equal to 3 of type tally\nsay t",
        );
        check_ok(
            "a type called count is a number\na type called tally is a count\nmake t equal to 3 of type tally\nsay t",
        );
        // Transparent: a score IS a number (ordering compares numbers).
        check_ok(
            "a type called score is a number\nmake s equal to 2 of type score\nif s is greater than 1\n    say \"big\"",
        );
        // Errors: duplicate alias / shadowing a real type / self-reference /
        // a two-alias cycle.
        check_err("a type called score is a number\na type called score is a text", "E0371");
        check_err(
            "structure player\n    has name of type text\n\na type called player is a number",
            "E0371",
        );
        check_err("a type called score is a score", "E0372");
        check_err("a type called left is a right\na type called right is a left", "E0372");
        // Unknown target: the words after `is a` name no type.
        check_err("a type called gem is a jewel", "E0373");
    }

    #[test]
    fn files_and_json_check() {
        check_ok(
            "use files\nuse json\nattempt write file \"hi\" at \"out.txt\" if it fails then\n    say problem\notherwise\n    say result\nmake data equal to a map from \"a\" to 1\nsay json text from data",
        );
        // Unhandled can-fail file call → E0302 (13.1's rule applies to
        // stdlib failures too).
        check_err("use files\nwrite file \"hi\" at \"out.txt\"", "E0302");
    }

    #[test]
    fn text_module_surface() {
        check_ok(
            "make parts equal to split \"a,b,c\" and \",\"\nsay size of parts\nmake joined equal to join parts\nsay contains \"hello\" and \"ell\"",
        );
    }

    // ------------------------------------------------------------------
    // M1 spec-extract corpus (docs/13 §3/§4 + 00 §7.12/§8.5/§11/§13.1).
    // One entry per spec sentence/row: the extract is the corpus index, and
    // this block is what makes its M1 rows "specification in executable
    // form" (docs/00 §29's corpus rule).
    // ------------------------------------------------------------------

    #[test]
    fn m1_corpus_kind_three_forms() {
        // 7.12's own exemplar: variant with fields, multi-field variant,
        // fieldless variant.
        check_ok(SHAPE_PROGRAM);
        check_ok(
            "kind shape\n    is a circle with radius of type number\n    is a rectangle with width of type number and height of type number\n    is a blank\n\nfunction describe\n    takes shape called s\n    returns text\n    match s\n        when a circle with radius r\n            gives back \"round, {r}\"\n        when a rectangle with width w and height h\n            gives back \"boxy\"\n        when blank\n            gives back \"empty\"",
        );
    }

    #[test]
    fn m1_corpus_match_on_every_pattern_form() {
        // 7.15's pattern production: literal, binding name, variant
        // destructure, nothing, something with value, pair.
        check_ok(
            "match 3\n    when 3\n        say \"three\"\n    when other\n        say other",
        );
        check_ok(SHAPE_PROGRAM);
        check_ok(
            "make m equal to first of a list of \"x\"\nmatch m\n    when nothing\n        say \"empty\"\n    when something with value v\n        say v",
        );
        check_ok(
            "make p equal to a pair of 1 and 2\nmatch p\n    when a pair of x and y\n        make total equal to x plus y\n        say total",
        );
        // `otherwise` closes a non-exhaustive literal match (7.15).
        check_ok(
            "match \"go\"\n    when \"stop\"\n        say \"halted\"\n    otherwise\n        say \"rolling\"",
        );
    }

    #[test]
    fn m1_corpus_exhaustiveness_names_missing_case() {
        // 7.12: "a `match` that misses a case is a compile error with the
        // missing case named" — the diagnostic text carries the name.
        let src = "kind shape\n    is a circle with radius of type number\n    is a blank\n\nfunction describe\n    takes shape called s\n    match s\n        when a circle with radius r\n            gives back \"round\"";
        let diags = check_err(src, "E0360");
        let shown = diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n");
        assert!(shown.contains("blank"), "missing case must be named: {shown}");
        // Option matches must cover both halves (8.5).
        check_err(
            "make m equal to first of a list of \"x\"\nmatch m\n    when nothing\n        say \"empty\"",
            "E0360",
        );
    }

    #[test]
    fn m1_corpus_option_both_spellings_and_construction() {
        // 8.5/R-18: `T?` and `a T or nothing` are one type; values are
        // `nothing` (13/8.5) and `something with value v` (docs/14 G-21).
        check_ok(
            "function head\n    takes a list of text called xs\n    returns a text or nothing\n    if size of xs is equal to 0\n        gives back nothing\n    gives back something with value \"head\"\n\nsay head of a list of \"a\"",
        );
        check_ok(
            "function head\n    takes a list of text called xs\n    returns text?\n    gives back nothing\n\nmake m equal to head of a list of \"a\"\nsay m",
        );
        // `something with value` against a non-option is refused (8.5).
        check_err(
            "match 5\n    when something with value v\n        say v\n    otherwise\n        say \"other\"",
            "E0365",
        );
    }

    #[test]
    fn m1_corpus_flow_narrowing_and_say_printing() {
        // 8.5: `say`/interpolation print the contained value or `nothing`
        // (D-34); `is nothing`/`is something` comparisons are legal (S-13).
        check_ok(
            "make m equal to first of a list of \"x\"\nif m is nothing\n    say \"empty\"\notherwise\n    say \"first is {m}\"",
        );
        check_ok("make m equal to first of a list of \"x\"\nif m is something\n    say \"got one\"\notherwise\n    say \"none\"");
    }

    #[test]
    fn m1_corpus_lambdas_all_forms() {
        // 7.15's lambda production: bare name passes the function itself;
        // `it` comparison; `taking … giving back` inline; block lambda.
        check_ok("function double\n    takes number called n\n    returns number\n    gives back n times 2\n\nmake raised equal to map a list of 1, 2 using double");
        check_ok("make raised equal to map a list of 1, 2 using it plus 5");
        check_ok("make raised equal to map a list of 1, 2 using taking n giving back n times 2");
        // §11.1's block lambda: `a function taking n` opens an indented body.
        check_ok("make double equal to a function taking n\n    gives back n times 2\n\nmake raised equal to map a list of 1, 2 using double");
    }

    #[test]
    fn m1_corpus_combinators_with_labeled_start() {
        // 11.2's four exemplars, including `combine … with start 0 using …`.
        check_ok("make scores equal to a list of 85, 92, 78\nmake raised equal to map scores using it plus 5");
        check_ok("make scores equal to a list of 85, 92, 78\nmake passing equal to keep scores where it is at least 80");
        check_ok("make scores equal to a list of 85, 92, 78\nmake total equal to combine scores with start 0 using start plus it");
    }

    #[test]
    fn m1_corpus_files_all_operations() {
        // §19.1 files surface; every op can fail (13.1) and must be handled.
        check_ok(
            "use files\nattempt open file at \"data.txt\" if it fails then\n    say problem\notherwise\n    say result",
        );
        check_ok(
            "use files\nattempt write file \"a\" at \"b.txt\" if it fails then\n    say problem\notherwise\n    say result",
        );
        check_ok(
            "use files\nattempt append file \"a\" at \"b.txt\" if it fails then\n    say problem\notherwise\n    say result",
        );
        check_ok(
            "use files\nattempt delete file at \"b.txt\" if it fails then\n    say problem\notherwise\n    say result",
        );
        check_ok(
            "use files\nmake p equal to \"b.txt\"\nmake here equal to file exists at p\nmake big equal to file size at p\nsay here",
        );
        // Unhandled can-fail file call → E0302 (13.1's compile error).
        check_err("use files\nwrite file \"hi\" at \"out.txt\"", "E0302");
        check_err("use files\nsay open file at \"gone.txt\"", "E0302");
    }

    #[test]
    fn m1_corpus_json_round_trip() {
        // The json module's two operations (§19.1): `json from text` can
        // fail; `json text from value` formats (S-9's map shape). The parse
        // input is read from a file (§13.1's honest path: files then json),
        // since a JSON object literal inside Lagom text would need `{}`
        // escapes the interpolation grammar reads as placeholders.
        check_ok(
            "use files\nuse json\nattempt open file at \"data.json\" if it fails then\n    say problem\notherwise\n    attempt json from result if it fails then\n        say problem\n    otherwise\n        say size of result\nmake data equal to a map from \"a\" to 1\nsay json text from data",
        );
        // Unhandled parse → E0302.
        check_err("use json\nmake data equal to json from \"bad\"", "E0302");
    }

    #[test]
    fn m1_corpus_result_propagation_forms() {
        // 13.1: the three handling forms, including `and pass the problem on`
        // inside another `can fail` function.
        check_ok(
            "function risky\n    can fail\n    fail with \"boom\"\n\nattempt risky if it fails then\n    say problem\notherwise\n    say result",
        );
        check_ok(
            "function risky\n    can fail\n    fail with \"boom\"\n\nattempt risky as trouble\n    say trouble",
        );
        check_ok(
            "function risky\n    can fail\n    fail with \"boom\"\n\nfunction caller\n    can fail\n    attempt risky and pass the problem on\n\nattempt caller if it fails then\n    say problem\notherwise\n    say result",
        );
    }

    #[test]
    fn m1_corpus_error_kinds_are_ordinary_kinds() {
        // 13.4: a `kind` of failures matched in a `when` — the one machinery.
        // The variant line carries §7.12's article form (`is a missing …`,
        // docs/14 G-24 — §13.4's exemplar drops the article; the normative
        // §7.12 form wins).
        check_ok(
            "kind file problem\n    is a missing with path of type text\n    is a denied with path of type text\n\nfunction explain\n    takes file problem called p\n    returns text\n    match p\n        when a missing with path f\n            gives back \"no file at {f}\"\n        when a denied with path f\n            gives back \"denied: {f}\"",
        );
    }

    #[test]
    fn m1_corpus_call_grammar_suffix_order() {
        // §7.15's call production: with/using/where suffixes, fixed order,
        // each at most once — plus the `it` reserved word.
        check_ok("make big ones equal to keep a list of \"aaaa\", \"b\" where size of it is greater than 2");
        check_ok("make raised equal to map a list of 1, 2 using it plus 5\nmake doubled equal to map raised using taking n giving back n times 2");
        // `it` is a reserved word (11.1): the parser rejects it as a
        // declaration name before sema ever sees it.
        check_err("make it equal to 5", "E0208");
    }

    #[test]
    fn m1_corpus_diagnostic_surface_covers_new_features() {
        // The M1 diagnostics: wrong-typed pattern (E0362), field-less
        // variant pattern (E0363), unknown pattern field (E0364), option
        // mismatch (E0365), non-function using (E0366), unknown variant
        // (E0367), missing variant field (E0368), combinator type errors
        // (E0369/E0370).
        check_err(
            "match 5\n    when \"five\"\n        say \"five\"\n    otherwise\n        say \"other\"",
            "E0362",
        );
        check_err(
            "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when circle\n        say \"round\"",
            "E0363",
        );
        check_err(
            "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when a circle with diameter d\n        say d",
            "E0364",
        );
        check_err(
            "kind shape\n    is a circle with radius of type number\nmake s equal to a circle with radius 1\nmatch s\n    when a triangle with side t\n        say t",
            "E0367",
        );
        // An unknown construction name (neither structure nor variant) is
        // E0347, with a pointer at the variant alternative.
        check_err(
            "kind shape\n    is a circle with radius of type number\nmake s equal to a triangle with side 1",
            "E0347",
        );
        check_err(
            "kind shape\n    is a circle with radius of type number\nmake s equal to a circle",
            "E0368",
        );
        check_err(
            "make broken thing equal to 5\nmake also broken equal to map a list of 1 using broken thing",
            "E0366",
        );
        check_err("make broken equal to map 5 using it plus 1", "E0370");
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

    /// M2: suggestions must not mislead. `it` is contained in `split`, but
    /// proposing `split` for an unknown 2-letter name sends the student the
    /// wrong way — containment requires a 3+ character name, and the typo
    /// path (`scoer` → `score`) still suggests.
    #[test]
    fn short_unknown_names_get_no_false_suggestion() {
        let diags = check_err("make split equal to \"a, b\"\nsay it", "E0344");
        let e = diags.iter().find(|d| d.code == "E0344").expect("E0344");
        assert!(e.fix.is_none(), "no misleading suggestion, got: {:?}", e.fix);
        // And the typo path still works.
        let diags = check_err("make score equal to 1\nsay scoer", "E0344");
        let e = diags.iter().find(|d| d.code == "E0344").expect("E0344");
        assert!(e.fix.as_deref().unwrap_or_default().contains("score"), "suggests the real name: {:?}", e.fix);
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

    // G-20 (docs/14): §8.4 wins — `text plus text` is concatenation.
    // Mixing text with a non-text operand stays an error (7.7's
    // interpolation is the teaching form for that).
    #[test]
    fn text_plus_text_concatenates() {
        check_ok("make greeting equal to \"Hello, \" plus \"world\"");
        check_err("make bad equal to \"age: \" plus 5", "E0357");
    }

    #[test]
    fn decimal_to_number_is_not_implicit() {
        check_err("\nfunction f\n    returns a number\n    gives back 1.5", "E0360");
    }

    #[test]
    fn returns_inferred_when_absent() {
        // §7.8: `returns` is optional; the body's gives back infers it (D-11).
        check_ok("\nfunction count down\n    takes number called n\n    if n is at most 0\n        gives back 0\n    gives back count down n minus 1");
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
        check_ok("\nfunction add\n    takes number called a\n    takes number called b\n    returns a number\n    gives back a plus b\n\nsay add 2 and 3");
        check_err("\nfunction add\n    takes number called a\n    returns a number\n    gives back a\n\nsay add 2 and 3", "E0354");
    }

    #[test]
    fn recursion_resolves_file_relative() {
        check_ok("\nfunction count down\n    takes number called n\n    if n is at most 0\n        gives back 0\n    gives back count down n minus 1");
    }

    #[test]
    fn unhandled_can_fail_call_is_compile_error() {
        check_err("\nfunction divide\n    takes number called top\n    takes number called bottom\n    returns a number\n    can fail\n    gives back top divided by bottom\n\nsay divide 10 and 0", "E0302");
    }

    #[test]
    fn attempt_handles_can_fail() {
        check_ok("\nfunction divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    if bottom is equal to 0\n        fail with \"cannot divide by zero\"\n    gives back top divided by bottom\n\nattempt divide 10 and 0 if it fails then\n    say problem\notherwise\n    say result");
    }

    #[test]
    fn as_binding_and_propagation() {
        // G-14: the spec example's `returns` corrects to `a decimal` (D-10).
        check_ok("\nfunction divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    gives back top divided by bottom\n\nattempt divide 1 and 2 as problem\n    say problem");
        // Propagation requires the enclosing function to declare can fail.
        check_err("\nfunction divide\n    takes number called top\n    returns a decimal\n    can fail\n    gives back top\n\nfunction inner\n    attempt divide 1 and 2 and pass the problem on", "E0332");
    }

    #[test]
    fn bare_attempt_needs_can_fail_function() {
        check_err("\nfunction divide\n    takes number called top\n    returns a number\n    can fail\n    gives back top\n\nmake x equal to attempt divide 1 and 2", "E0332");
    }

    #[test]
    fn fail_with_needs_can_fail() {
        check_err("function f\n    fail with \"no\"", "E0337");
        check_ok("function f\n    can fail\n    fail with \"yes\"");
    }

    #[test]
    fn returns_contract() {
        check_err("\nfunction f\n    returns a number", "E0333");
        // `gives back` outside a function is the E0336 case; inside a function
        // without `returns`, the type is inferred (7.8/D-11).
        check_err("gives back 1", "E0336");
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
            "\nfunction divide\n    takes number called top\n    returns a decimal\n    can fail\n    gives back top\n\nattempt divide 1 as problem\n    make problem equal to \"x\"\n    say problem\notherwise\n    make result equal to 1\n    say result",
        );
        assert!(
            out.diags.items.iter().any(|d| d.code == "W0332"),
            "expected the shadow lint"
        );
    }
}

/// G-22 (docs/14): does this expression a `call` that has exactly one
/// positional argument and nothing else? The left operand of an `and` in
/// that shape is a two-argument flowing call in progress (`split line and
/// ","`), not a boolean conjunction. Nested calls (`say split line and
/// ","`) flatten through the same check when their own shape qualifies.
fn is_single_arg_call(e: &Expr) -> bool {
    match e {
        Expr::Call(c) => {
            c.first.is_some()
                && c.preps.is_empty()
                && c.and_args.is_empty()
                && c.with_args.is_empty()
                && c.using_arg.is_none()
                && c.where_expr.is_none()
        }
        Expr::Group { inner, .. } => is_single_arg_call(inner),
        _ => false,
    }
}

/// G-22 (docs/14): an and-arg the parser wrongly closed with the sentence's
/// `using`/`where` suffix (`combine xs and seen using f` — `starts_argument`
/// treats `using` as an opener, so `seen using f` parses as a call). Per R-3
/// those suffixes belong to the WHOLE sentence's call, so an and-arg whose
/// only arguments ARE those suffixes is a plain value read; return the suffix
/// and the stripped name. Returns `None` for anything else.
fn and_arg_with_suffix(e: &Expr) -> Option<(Expr, Option<Box<Expr>>, Option<Box<Expr>>)> {
    let Expr::Call(c) = e else { return None };
    if c.first.is_some() || !c.preps.is_empty() || !c.and_args.is_empty() || !c.with_args.is_empty() {
        return None;
    }
    if c.using_arg.is_none() && c.where_expr.is_none() {
        return None;
    }
    if c.callee.words.len() != 1 {
        return None;
    }
    Some((
        Expr::Name { name: c.callee.clone(), span: c.callee.span },
        c.using_arg.clone(),
        c.where_expr.clone(),
    ))
}

pub fn expr_span(e: &Expr) -> Span {    match e {
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
        | Expr::VariantLit { span, .. }
        | Expr::SomeValue { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::AttemptExpr { span, .. } => *span,
        Expr::Call(c) => c.span,
    }
}
