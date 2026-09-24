// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The Lagom AST (M0 subset).
//!
//! Spec anchor: 00 §7.15 (normative grammar), docs/13 §1–2 (M0 extract).
//! Every node carries a [`Span`] obtained from the lexer — diagnostics are
//! source-aware from the first token onward (brief: spans first-class).

use lagom_diagnostics::Span;

// ---------------------------------------------------------------------------
// Program structure
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub enum Item {
    Function(FunctionDecl),
    Structure(StructureDecl),
    /// `class counter … has … can …` (10.2) — a stateful, identity-bearing
    /// object (composition-first, 10.1: classes are taught after structs).
    Class(ClassDecl),
    /// `kind shape … is a circle with radius …` (7.12) — the sum type.
    Kind(KindDecl),
    /// `interface drawable … can draw …` (10.6) — the substitution contract.
    Interface(InterfaceDecl),
    /// `a type called score is a number` (8.4) — the type alias.
    TypeAlias(TypeAliasDecl),
    Test(TestDecl),
    Use(UseDecl),
    Stmt(Stmt),
}

/// `a type called score is a number` (8.4): a new name for an existing type.
/// Aliases are *transparent* — `score` and `number` are the same type to the
/// checker, and no runtime stage ever sees the alias (it resolves in sema).
#[derive(Debug)]
pub struct TypeAliasDecl {
    pub name: Name,
    pub ty: TypeExpr,
    pub span: Span,
}

impl TypeAliasDecl {
    /// The target type's span (for diagnostics pointing at `is a <type>`).
    pub fn ty_span(&self) -> Span {
        match &self.ty {
            TypeExpr::User(n) => n.span,
            TypeExpr::List(t) | TypeExpr::Map(t, _) | TypeExpr::Pair(t, _) | TypeExpr::Channel(t) => match t.as_ref() {
                TypeExpr::User(n) => n.span,
                _ => self.name.span,
            },
            _ => self.name.span,
        }
    }
}

/// `kind` — an algebraic data type (7.12): one variant per line, each with
/// its name (the variant literal is the name) and optional `with`-fields.
#[derive(Debug)]
pub struct KindDecl {
    pub name: Name,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

/// `is a circle with radius of type number` — one `kind` variant. A variant
/// with no fields is a literal (`blank`); one with fields constructs with
/// `a circle with radius 5`.
#[derive(Debug)]
pub struct VariantDecl {
    pub name: Name,
    /// (field, type, span) in declaration order.
    pub fields: Vec<(Name, TypeExpr, Span)>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Declarations
// ---------------------------------------------------------------------------

/// `function calculate average … takes … returns … block` (7.8).
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: Name,
    pub params: Vec<Param>,
    pub returns: Option<TypeExpr>,
    pub can_fail: bool,
    pub body: Block,
    pub span: Span,
}

/// `takes number called x` / `takes a list of numbers called scores` / `takes number of correct answers` (7.8).
#[derive(Debug, Clone)]
pub struct Param {
    pub name: Name,
    pub ty: TypeExpr,
    pub span: Span,
}

/// `class counter … has … can … block` (10.2): fields plus methods. Methods
/// lower to receiver-first functions (R-2 — "methods are functions that take
/// the object first"), so this carries only the class surface; the parser
/// synthesizes each `can` body as a real `FunctionDecl` named `method <class>
/// <name>` — unspellable as a user call head because `method` is a reserved
/// word once a class exists.
#[derive(Debug)]
pub struct ClassDecl {
    pub name: Name,
    /// `has <field> of type <T>` lines, in declaration order.
    pub fields: Vec<FieldDecl>,
    /// `construction` clauses (10.3): named constructors, parsed as
    /// receiver-first functions (the `takes` clauses are the parameters;
    /// full field initialization is sema's Swift rule). Multiple clauses
    /// dispatch by argument names (D-14), never arity.
    pub constructions: Vec<FunctionDecl>,
    /// Methods: (plain name, receiver-first function synthesized by the parser).
    pub methods: Vec<(Name, FunctionDecl)>,
    /// `extends <class>` (10.5): the single base class, if any. Sema
    /// copy-redirects dispatch through the base chain — no field/method
    /// cloning in the AST.
    pub extends: Option<Name>,
    /// `does <interface>, <interface>` (10.6): conformance claims checked
    /// against the interface's requirement table.
    pub does: Vec<Name>,
    /// `before last reference disappears` clause (10.4): the parser
    /// synthesizes it as a receiver-first function named `deinit <class>`;
    /// at most one per class. Sema enforces R-20.3 (finalizers cannot fail).
    pub deinit: Option<FunctionDecl>,
    pub span: Span,
}

/// `interface drawable … can draw …` (10.6): a named set of method
/// requirements. A `can` line with a body is a default implementation —
/// conforming classes inherit it unless they define their own.
#[derive(Debug)]
pub struct InterfaceDecl {
    pub name: Name,
    /// (plain name, optional default body as a receiver-first function of
    /// the interface's own name).
    pub methods: Vec<(Name, Option<FunctionDecl>)>,
    pub span: Span,
}

/// `structure player … has name of type text …` (7.11).
#[derive(Debug)]
pub struct StructureDecl {
    pub name: Name,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug)]
pub struct FieldDecl {
    pub name: Name,
    pub ty: TypeExpr,
    pub span: Span,
}

/// `test "name" … block` with `check that …` inside (5.1, 26.3).
#[derive(Debug)]
pub struct TestDecl {
    pub name: String,
    pub name_span: Span,
    pub body: Block,
    pub span: Span,
}

/// `use math for square root, floor` / `use drawing` (7.13).
#[derive(Debug)]
pub struct UseDecl {
    pub module: Name,
    pub imports: Option<Vec<Name>>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Statements (docs/13 §1)
// ---------------------------------------------------------------------------

/// A block: the indented body of a function, test, branch, or attempt tail.
/// Owned here (not in the parser) because statements and declarations embed it.
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

impl Block {
    pub fn new(stmts: Vec<Stmt>, span: Span) -> Self {
        Block { stmts, span }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// `make x equal to …` / `make changing score equal to …` (7.2). The
    /// `of type` annotation is optional (7.10's own example uses it — docs/14
    /// G-11: the annotation suffix is examplar-normative, added to the
    /// extract's make production).
    Make {
        mutable: bool,
        name: Name,
        value: Expr,
        annotation: Option<TypeExpr>,
        span: Span,
    },
    /// `set <target> to …` (7.2/7.6).
    Set {
        target: Target,
        value: Expr,
        span: Span,
    },
    /// `increase <target> by …` / `decrease <target> by …` (7.2).
    Change {
        decrease: bool,
        target: Target,
        value: Expr,
        span: Span,
    },
    /// `if … ` with `otherwise if` / `otherwise` chains (7.4).
    If {
        branches: Vec<(Expr, Block)>,
        otherwise: Option<Block>,
        span: Span,
    },
    Repeat(Repeat),
    Stop { span: Span },
    Next { span: Span },
    /// `gives back <expr>` (7.8).
    GiveBack { value: Expr, span: Span },
    /// `fail with <expr>` (13.1).
    FailWith { value: Expr, span: Span },
    /// `attempt … [tail]` (13.1, R-20.1: one production, statement and expression).
    Attempt { expr: Box<Expr>, tail: Option<AttemptTail>, span: Span },
    /// `check that <comparison>` (5.1).
    CheckThat { expr: Expr, span: Span },
    /// `match <expr> { when <pattern> block } [otherwise block]` (7.12/7.15).
    Match { scrutinee: Expr, arms: Vec<(Pattern, Block)>, otherwise: Option<Block>, span: Span },
    /// A bare flowing call used for effect: `bump c` (7.15 exprstmt).
    ExprStmt { expr: Expr, span: Span },
    /// `start a task` block { `keep going` suffix } — one spawn per statement
    /// (14.2): the body runs concurrently with the spawning function; the
    /// region joins at `wait for all tasks` or function exit.
    StartTask {
        body: Block,
        /// `keep going` — this task's failure must not fail the region.
        keep_going: bool,
        span: Span,
    },
    /// `wait for all tasks` — the explicit join (14.2). The reserved
    /// phrase-token needs no payload; the join's failure surface is the
    /// supervision rule.
    WaitForAllTasks { span: Span },
}

/// Patterns (7.15's `pattern` production, M1):
/// `pattern = literal | name | "a" usertype [with name [and name] …] | "nothing"
///          | "something with value" pattern | pair`.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// A literal: `when 0`, `when "quit"`.
    Literal { value: PatternLiteral, span: Span },
    /// A bare binding name, or a variant-literal name (`when blank`),
    /// `when nothing`, `when something`.
    Name { name: Name },
    /// `a circle with radius r and height h` — variant destructuring; field
    /// entries hold the *sub-pattern* for each named field.
    Variant { name: Name, fields: Vec<(Name, Pattern)>, span: Span },
    /// `something with value <pattern>` — the option destructuring (8.5).
    Something { inner: Box<Pattern>, span: Span },
    /// `a pair of first and second` — pair destructuring: the two sub-patterns.
    Pair { first: Box<Pattern>, second: Box<Pattern>, span: Span },
    /// The catch-all `otherwise` arm has no pattern.
    Wildcard,
}

/// The literal forms a pattern may match (comparisons against the scrutinee).
#[derive(Debug, Clone)]
pub enum PatternLiteral {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Nothing,
}

/// Loop forms (7.5): count / while / for-each.
#[derive(Debug, Clone)]
pub enum Repeat {
    /// `repeat 10 times using i` — literal count only (docs/14 G-3: variable
    /// counts are expressed with `repeat while` per the frozen grammar).
    Count { times: (i64, Span), binding: Option<Name>, body: Block, span: Span },
    /// `repeat while <expr>`.
    While { cond: Expr, body: Block, span: Span },
    /// `repeat for each item in things` or the index form
    /// `repeat for each word, position in words` (docs/14 G-1).
    ForEach {
        item: Name,
        index: Option<Name>,
        iter: Expr,
        body: Block,
        span: Span,
    },
}

/// `attempt` tails (7.15): propagation / if-it-fails / as-binding.
#[derive(Debug, Clone)]
pub enum AttemptTail {
    /// `and pass the problem on` (statement-only; phrase-token).
    Propagate,
    /// `if it fails then <block> [otherwise <block>]` — binds `problem`/`result` (13.1).
    IfItFails { then_block: Block, otherwise: Option<Block> },
    /// `as <name> <block> [otherwise <block>]` — binds the error under a chosen name.
    As { name: Name, block: Block, otherwise: Option<Block> },
}

/// Assignment/set target (7.15 `target = name { prep additive }`).
#[derive(Debug, Clone)]
pub struct Target {
    pub base: Name,
    pub accessors: Vec<Accessor>,
    pub span: Span,
}

/// One prepositional accessor step: `at 2` (index) or `of p` (field).
#[derive(Debug, Clone)]
pub enum Accessor {
    At { index: Expr, span: Span },
    Of { field: Name, span: Span },
}

// ---------------------------------------------------------------------------
// Expressions (docs/13 §2, precedence ladder of 7.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Expr {
    Int { value: i64, span: Span },
    Float { value: f64, span: Span },
    Text { value: String, span: Span },
    Bool { value: bool, span: Span },
    /// `nothing` — the option sentinel (8.5); the value of `nothing?` results.
    Nothing { span: Span },
    /// A (possibly multi-word) name: `score`, `square root`, `top score`.
    Name { name: Name, span: Span },
    /// Interpolation piece of a text literal: kept whole here, desugared to
    /// `format` calls in HIR (7.7). `parts` alternates literal text and `{expr}`.
    Interp { parts: Vec<InterpPart>, span: Span },
    /// Grouping: `( expr )`.
    Group { inner: Box<Expr>, span: Span },
    /// Unary minus.
    Neg { inner: Box<Expr>, span: Span },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr>, span: Span },
    /// `not <expr>` (7.3).
    Not { inner: Box<Expr>, span: Span },
    /// A flowing call (7.9/7.15): `name [arg1] {prep argN} {and argN} {with …}`.
    /// Boxed: `CallExpr` → `Arg` → `Expr` would otherwise be infinite-sized.
    Call(Box<CallExpr>),
    /// The option construction (8.5/G-21): `something with value v`. At the
    /// value level it lowers to `v` itself (options are value-or-`nothing`,
    /// D-34); sema types it as an option of the value's type.
    SomeValue { value: Box<Expr>, span: Span },
    /// `a list of 1, 2, 3` (7.6).
    ListLit { elements: Vec<Expr>, span: Span },
    /// `a map from "ana" to 11, "bo" to 12` — keys are primaries (R-5).
    MapLit { entries: Vec<(Expr, Expr)>, span: Span },
    /// `a pair of 3 and 4` (7.6).
    PairLit { first: Box<Expr>, second: Box<Expr>, span: Span },
    /// `a player with name "bo" and score 0` (7.11) — labeled construction.
    StructLit { name: Name, fields: Vec<(Name, Expr)>, span: Span },
    /// `a new <class> with <field> <value> [and …]` (10.2/10.3) — class
    /// construction. Sema checks full field initialization (the Swift rule,
    /// 10.3) and types the result as the class; both backends produce an
    /// ARC'd reference object (9.3).
    NewObject { name: Name, fields: Vec<(Name, Expr)>, span: Span },
    /// `a <kind-variant> with <field> <additive> and …` — variant construction
    /// (7.12): the same reader shape as structlit, resolved against `kind`
    /// tables by sema.
    VariantLit { name: Name, fields: Vec<(Name, Expr)>, span: Span },
    /// Bare attempt used as an expression (13.1): `(attempt divide 10 and 0) plus 1`.
    AttemptExpr { expr: Box<Expr>, span: Span },
    /// A lambda (11.1): either the block form (`a function taking n … body`)
    /// or the inline form (`taking n giving back n times 2` — body one
    /// comparison). `it` is resolved by sema, never stored here.
    Lambda { params: Vec<Name>, body: LambdaBody, span: Span },
    /// `a channel of T` as a value (14.3): the type expression IS the
    /// construction — a channel is made by naming it. The element type rides
    /// along for `fmt` round-tripping; sema erases it to the message type.
    ChannelLit { elem: Box<TypeExpr>, span: Span },
}

/// A lambda's body (R-3, 11.1): the block form owns statements; the inline
/// form owns exactly one comparison-level expression.
#[derive(Debug, Clone)]
pub enum LambdaBody {
    Block(Block),
    Inline(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    /// True division — promotes to decimal (D-7).
    Div,
    /// Floor division (D-7).
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
pub enum InterpPart {
    Lit(String),
    Expr(Expr),
}

/// A call site: the callee name and its argument groups, in the order given.
#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Name,
    /// First positional argument, if present (`ask "…"` / `divide 10`).
    pub first: Option<Box<Arg>>,
    /// Prepositional arguments in source order (`from 1`, `to 6`, `at 0`, `of x`).
    pub preps: Vec<(Prep, Arg)>,
    /// `and`-separated additional arguments (`divide 10 and 0`).
    pub and_args: Vec<Arg>,
    /// `with <name> <additive>` labeled suffixes (7.9).
    pub with_args: Vec<(Name, Arg)>,
    /// `using <lambda>` — the function argument (R-3/11.2).
    pub using_arg: Option<Box<Expr>>,
    /// `where <orexpr>` — the filter sugar; desugars in sema to
    /// `using taking it giving back <expr>` (R-3's one desugaring rule).
    pub where_expr: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prep {
    Of,
    At,
    From,
    To,
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub expr: Box<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Names & types
// ---------------------------------------------------------------------------

/// A name: one or more words (`score`, `square root`, `top score`). Words are
/// stored individually so sema can apply greedy multi-word resolution (7.0.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    pub words: Vec<String>,
    pub span: Span,
}

impl Name {
    pub fn display(&self) -> String {
        self.words.join(" ")
    }
}

/// Type expressions (docs/13 §2 `type` production, M0 subset of 7.10).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Number,
    Decimal,
    Text,
    Boolean,
    List(Box<TypeExpr>),
    Map(Box<TypeExpr>, Box<TypeExpr>),
    /// `a channel of T` (14.3): a typed FIFO channel.
    Channel(Box<TypeExpr>),
    Pair(Box<TypeExpr>, Box<TypeExpr>),
    /// A user-defined structure or kind name.
    User(Name),
    /// `T?` — the compact option spelling (8.5, R-18). One desugaring rule:
    /// the same type as `a T or nothing`.
    OptionT(Box<TypeExpr>),
    /// The type parameter (12.1/12.2): `anything` (implicit, inferred from
    /// use) and `some type` (the explicit spelling of the same word) are the
    /// one concept — frozen grammar words, not user names.
    /// The type parameter (12.1/12.2): `anything` / `some type`, optionally
    /// constrained — `… that does <interface>` (12.3). `None` is the
    /// unconstrained parameter; `Some(name, span)` names the interface every
    /// call-site argument must satisfy. (The 12.3 existential — the same
    /// words read as a value type — is the same surface; v0.1 checks it as
    /// the constrained parameter it always appears as.)
    TypeParam {
        iface: Option<(String, Span)>,
    },
    /// Missing annotation (inferred).
    Inferred,
}

impl TypeExpr {
    /// Rendering used by diagnostics ("a list of numbers", 7.10 word style).
    pub fn display(&self) -> String {
        match self {
            TypeExpr::Number => "number".into(),
            TypeExpr::Decimal => "decimal".into(),
            TypeExpr::Text => "text".into(),
            TypeExpr::Boolean => "boolean".into(),
            TypeExpr::List(t) => format!("a list of {}", t.display()),
            TypeExpr::Map(k, v) => format!("a map from {} to {}", k.display(), v.display()),
            TypeExpr::Channel(t) => format!("a channel of {}", t.display()),
            TypeExpr::Pair(a, b) => format!("a pair of {} and {}", a.display(), b.display()),
            TypeExpr::User(n) => n.display(),
            TypeExpr::OptionT(t) => format!("{}?", t.display()),
            TypeExpr::TypeParam { iface: None } => "anything".into(),
            TypeExpr::TypeParam {
                iface: Some((i, _)),
            } => format!("anything that does {i}"),
            TypeExpr::Inferred => "inferred".into(),
        }
    }
}
