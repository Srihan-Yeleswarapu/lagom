//! The Lagom parser (M0 subset).
//!
//! Spec anchor: 00 §7.15 (normative grammar), docs/13 §1–2 (M0 extract).
//!
//! Recursive descent over the lexer's item stream, with block structure from
//! Indent/Dedent. Error recovery: on a statement-level error the parser skips
//! to the next Newline (region commits, 21.1) so one bad line does not hide
//! the errors in the rest of the program. No backtracking — the one documented
//! backtrack point (the `takes` filler-`of`, R-14 of doc 11) needs none at M0
//! because the type grammar cannot fail after its head word.

use lagom_ast as ast;
use lagom_ast::{
    Arg, AttemptTail, BinOp, Block, CallExpr, Expr, FieldDecl, FunctionDecl, InterpPart, Name,
    Param, Prep, Program, Repeat, Stmt, StructureDecl, Target, TestDecl, TypeExpr, UseDecl,
};
use lagom_diagnostics::{Diagnostic, Span};
use lagom_lexer::{lex, Item, Kw, Tok};

/// The AST's top-level item type. The lexer's `Item` (token-level) shadows it
/// in this module, so the AST type is used through this alias.
type AstItem = ast::Item;

/// Parse a source file into a [`Program`] plus any diagnostics.
pub fn parse(src: &str) -> (Program, Vec<Diagnostic>) {
    let lexed = lex(src);
    let mut p = Parser {
        src,
        items: lexed.items,
        sentinel: (Item::Tok(Tok::Eof), Span::default()),
        pos: 0,
        errors: lexed.errors,
    };
    let program = p.program();
    (program, p.errors)
}

struct Parser<'src> {
    /// The original source, for slicing exact text into teaching diagnostics.
    src: &'src str,
    items: Vec<(Item, Span)>,
    /// Returned by `peek` at one-past-the-end (defensive Eof).
    sentinel: (Item, Span),
    pos: usize,
    errors: Vec<Diagnostic>,
}

/// Sentinel for error recovery: parse functions return `Err(())` after pushing a
/// diagnostic; statement-level callers then skip to the next Newline.
type PResult<T> = Result<T, ()>;

impl<'src> Parser<'src> {
    // ------------------------------------------------------------------
    // Item stream helpers
    // ------------------------------------------------------------------

    fn peek(&self) -> (&Item, Span) {
        match self.items.get(self.pos) {
            Some((item, span)) => (item, *span),
            // Defensive: bump() keeps pos <= len, so this is only the position
            // after the final Eof — treat it as Eof via the stored sentinel.
            None => (&self.sentinel.0, self.sentinel.1),
        }
    }

    fn peek_kw(&self) -> Option<Kw> {
        match self.peek().0 {
            Item::Kw(k) => Some(*k),
            _ => None,
        }
    }

    fn peek_tok(&self) -> Option<&Tok> {
        match self.peek().0 {
            Item::Tok(t) => Some(t),
            _ => None,
        }
    }

    fn bump(&mut self) -> (Item, Span) {
        let cur = self.items[self.pos.min(self.items.len() - 1)].clone();
        if self.pos < self.items.len() {
            self.pos += 1;
        }
        cur
    }

    fn eat_kw(&mut self, kw: Kw) -> Option<Span> {
        if self.peek_kw() == Some(kw) {
            Some(self.bump().1)
        } else {
            None
        }
    }

    fn eat_tok(&mut self, tok: &Tok) -> Option<Span> {
        if matches!(self.peek_tok(), Some(t) if t == tok) {
            Some(self.bump().1)
        } else {
            None
        }
    }

    fn expect_kw(&mut self, kw: Kw, what: &str) -> PResult<Span> {
        if let Some(s) = self.eat_kw(kw) {
            Ok(s)
        } else {
            let (item, span) = self.peek();
            self.errors.push(
                Diagnostic::error("E0201", format!("I expected {what} here."), span)
                    .with_explanation(format!("This position needs the word `{}`.", kw_text(kw)))
                    .with_note(format!("found: {}", item_text(item))),
            );
            Err(())
        }
    }

    fn expect_tok(&mut self, tok: Tok, what: &str) -> PResult<Span> {
        if let Some(s) = self.eat_tok(&tok) {
            Ok(s)
        } else {
            let (item, span) = self.peek();
            self.errors.push(
                Diagnostic::error("E0201", format!("I expected {what} here."), span)
                    .with_note(format!("found: {}", item_text(item))),
            );
            Err(())
        }
    }

    fn at_newline(&self) -> bool {
        matches!(self.peek().0, Item::Tok(Tok::Newline))
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().0, Item::Tok(Tok::Eof))
    }

    fn at_dedent(&self) -> bool {
        matches!(self.peek().0, Item::Tok(Tok::Dedent))
    }

    /// Skip past the current logical line (error recovery, 21.1).
    fn skip_to_line_end(&mut self) {
        loop {
            match self.peek().0 {
                Item::Tok(Tok::Newline) => {
                    self.bump();
                    return;
                }
                Item::Tok(Tok::Eof) | Item::Tok(Tok::Dedent) => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Program & top level (docs/13 §1: toplevel = use | function | structure
    // | test | doccomment statement)
    // ------------------------------------------------------------------

    fn program(&mut self) -> Program {
        let mut items = Vec::new();
        loop {
            match self.peek().0 {
                Item::Tok(Tok::Eof) => break,
                Item::Tok(Tok::Newline) | Item::Tok(Tok::Dedent) => {
                    self.bump();
                }
                Item::Kw(Kw::Function) => match self.function_decl() {
                    Ok(f) => items.push(AstItem::Function(f)),
                    Err(()) => self.skip_decl(),
                },
                Item::Kw(Kw::Structure) => match self.structure_decl() {
                    Ok(s) => items.push(AstItem::Structure(s)),
                    Err(()) => self.skip_decl(),
                },
                Item::Kw(Kw::Test) => match self.test_decl() {
                    Ok(t) => items.push(AstItem::Test(t)),
                    Err(()) => self.skip_decl(),
                },
                Item::Kw(Kw::Use) => match self.use_decl() {
                    Ok(u) => items.push(AstItem::Use(u)),
                    Err(()) => self.skip_to_line_end(),
                },
                _ => match self.statement() {
                    Ok(Some(stmt)) => items.push(AstItem::Stmt(stmt)),
                    Ok(None) => {}
                    Err(()) => self.skip_to_line_end(),
                },
            }
        }
        Program { items }
    }

    /// Recovery for a failed *declaration*: skip balanced lines/blocks until the
    /// dedent back to top level (declarations own an indented block).
    fn skip_decl(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek().0 {
                Item::Tok(Tok::Eof) => return,
                Item::Tok(Tok::Indent) => {
                    depth += 1;
                    self.bump();
                }
                Item::Tok(Tok::Dedent) => {
                    self.bump();
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return;
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Types (7.10, M0 subset — docs/13 §2 `type`)
    // ------------------------------------------------------------------

    /// `type = ["a"|"an"] ( "number" | "decimal" | "text" | "boolean" | "a list of" type
    ///      | "a map from" type "to" type | "a pair of" type "and" type | usertype )`
    fn parse_type(&mut self, expecting: &str) -> PResult<TypeExpr> {
        // optional article (`takes a list of numbers called scores`,
        // `returns a number`, but `takes number called x` too)
        self.eat_kw(Kw::A);
        self.eat_kw(Kw::An);
        match self.peek_kw() {
            Some(Kw::Number) => {
                self.bump();
                Ok(TypeExpr::Number)
            }
            Some(Kw::Decimal) => {
                self.bump();
                Ok(TypeExpr::Decimal)
            }
            Some(Kw::Text) => {
                self.bump();
                Ok(TypeExpr::Text)
            }
            Some(Kw::Boolean) => {
                self.bump();
                Ok(TypeExpr::Boolean)
            }
            Some(Kw::AListOf) => {
                self.bump();
                let elem = self.parse_type("After `a list of`, write the element type.")?;
                Ok(TypeExpr::List(Box::new(elem)))
            }
            Some(Kw::AMapFrom) => {
                self.bump();
                let k = self.parse_type("`a map from` needs a key type.")?;
                self.expect_kw(Kw::To, "the word `to` (between the map's key and value types)")?;
                let v = self.parse_type("After `to`, write the value type.")?;
                Ok(TypeExpr::Map(Box::new(k), Box::new(v)))
            }
            Some(Kw::APairOf) => {
                self.bump();
                let a = self.parse_type("`a pair of` needs the first type.")?;
                self.expect_kw(Kw::And, "the word `and` (joining the pair's types)")?;
                let b = self.parse_type("After `and`, write the second type.")?;
                Ok(TypeExpr::Pair(Box::new(a), Box::new(b)))
            }
            _ => {
                // usertype: any name word run
                match self.peek_tok() {
                    Some(Tok::WordRun(_)) => Ok(TypeExpr::User(self.parse_name(expecting)?)),
                    _ => {
                        let (item, span) = self.peek();
                        self.errors.push(
                            Diagnostic::error("E0208", format!("I expected a type here ({expecting})."), span)
                                .with_note(format!("found: {}", item_text(item))),
                        );
                        Err(())
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Declarations (7.8 clauses, 7.11 structures, 5.1 tests, 7.13 use)
    // ------------------------------------------------------------------

    /// `function name {clause} block` (7.8, M0 clauses: takes/returns/can fail).
    /// Clause order is fixed (7.8: trivially parseable — that is the point).
    fn function_decl(&mut self) -> PResult<FunctionDecl> {
        let start = self.expect_kw(Kw::Function, "the word `function`")?;
        let name = self.parse_name("`function` must be followed by a name")?;
        // 7.8 layout: the header line ends, then the body block opens — and the
        // `takes`/`returns`/`can fail` clauses live at the *top* of that same
        // block, at body indent (7.8's example: clauses and statements share it).
        self.eat_newline_before_indent();
        let block_open = self.eat_tok(&Tok::Indent);
        if block_open.is_none() {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0202",
                "This function needs a body indented under its header.",
                span,
            ));
            return Err(());
        }
        let mut params = Vec::new();
        let mut returns = None;
        let mut can_fail = false;
        loop {
            match self.peek_kw() {
                Some(Kw::Takes) => {
                    self.bump();
                    params.push(self.takes_clause()?);
                }
                Some(Kw::Returns) => {
                    self.bump();
                    let ty = self.parse_type("After `returns`, write a type (like `a number`).")?;
                    returns = Some(ty);
                    self.end_of_line("the `returns` clause")?;
                }
                Some(Kw::CanFail) => {
                    self.bump();
                    can_fail = true;
                    self.end_of_line("the `can fail` clause")?;
                }
                _ => break,
            }
        }
        let body = self.block_after_open(block_open.unwrap())?;
        let span = start.to(body.span);
        Ok(FunctionDecl { name, params, returns, can_fail, body, span })
    }

    /// The statements of an already-opened block (function bodies: clauses have
    /// been consumed at the top of the block). Consumes the closing Dedent.
    fn block_after_open(&mut self, is: Span) -> PResult<Block> {
        let mut stmts = Vec::new();
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            match self.statement() {
                Ok(Some(s)) => stmts.push(s),
                Ok(None) => {}
                Err(()) => self.skip_to_line_end(),
            }
        }
        self.expect_tok(Tok::Dedent, "the end of this block (un-dent)")?;
        let span = is.to(self.peek().1);
        Ok(Block { stmts, span })
    }

    /// One `takes` clause (7.8): `takes number called x`, `takes a list of
    /// numbers called scores`, `takes number of correct answers` (filler `of`,
    /// name `correct answers`).
    fn takes_clause(&mut self) -> PResult<Param> {
        let start = self.peek().1;
        let ty = self.parse_type("After `takes`, write a type (like `a number`).")?;
        // filler `of` (7.8): only after a *complete atomic* type — exactly the
        // bounded backtrack point of R-14, and at M0 it needs no backtrack:
        // every compound type here consumes its own `of`/`to`/`and` first.
        self.eat_kw(Kw::Of);
        self.eat_kw(Kw::Called);
        let name = self.parse_name("`takes` needs a name for this parameter")?;
        self.end_of_line("the `takes` clause")?;
        let span = start.to(name.span);
        Ok(Param { name, ty, span })
    }

    /// `structure name ⏎ INDENT { has <name> of type <type> ⏎ } DEDENT` (7.11).
    fn structure_decl(&mut self) -> PResult<StructureDecl> {
        let start = self.expect_kw(Kw::Structure, "the word `structure`")?;
        let name = self.parse_name("`structure` must be followed by a name")?;
        self.end_of_line("the structure header")?;
        let fields = self.fields_block()?;
        let end = fields
            .last()
            .map(|f: &FieldDecl| f.span)
            .unwrap_or(start);
        Ok(StructureDecl { name, fields, span: start.to(end) })
    }

    /// The `has name of type T` field lines under a structure header (7.11).
    fn fields_block(&mut self) -> PResult<Vec<FieldDecl>> {
        let indent = self.eat_tok(&Tok::Indent);
        let Some(_is) = indent else {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0202",
                "This structure needs its fields indented under the header.",
                span,
            ));
            return Err(());
        };
        let mut fields = Vec::new();
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            let fstart = self.peek().1;
            if self.eat_kw(Kw::Has).is_none() {
                let (item, span) = self.peek();
                self.errors.push(
                    Diagnostic::error("E0201", "I expected `has` here.", span)
                        .with_explanation("Each field of a structure starts with the word `has`.")
                        .with_note(format!("found: {}", item_text(item))),
                );
                return Err(());
            }
            let fname = self.parse_name("`has` must be followed by the field's name")?;
            self.expect_kw(Kw::OfType, "the words `of type`")?;
            let ty = self.parse_type("After `of type`, write the field's type.")?;
            let tspan = ty_span(&ty);
            self.end_of_line("the field line")?;
            fields.push(FieldDecl { name: fname, ty, span: fstart.to(tspan) });
        }
        self.expect_tok(Tok::Dedent, "the end of this structure's field list (un-dent)")?;
        Ok(fields)
    }

    /// `test "name" block` (5.1, 26.3).
    fn test_decl(&mut self) -> PResult<TestDecl> {
        let start = self.expect_kw(Kw::Test, "the word `test`")?;
        let (name, name_span) = match self.peek_tok() {
            Some(Tok::Text(s)) => {
                let s = s.clone();
                let span = self.bump().1;
                (s, span)
            }
            _ => {
                let (item, span) = self.peek();
                self.errors.push(
                    Diagnostic::error("E0201", "A test needs a name in quotes.", span)
                        .with_fix("Write the test's name as text: test \"adds numbers\"")
                        .with_note(format!("found: {}", item_text(item))),
                );
                return Err(());
            }
        };
        let body = self.block("This test needs a body indented under its header.")?;
        let span = start.to(body.span);
        Ok(TestDecl { name, name_span, body, span })
    }

    /// `use math for square root, floor` / `use drawing` (7.13).
    fn use_decl(&mut self) -> PResult<UseDecl> {
        let start = self.expect_kw(Kw::Use, "the word `use`")?;
        let module = self.parse_name("`use` must be followed by a module name")?;
        let imports = if self.eat_kw(Kw::For).is_some() {
            let mut names = vec![self.parse_name("`use … for` needs at least one name")?];
            while self.eat_tok(&Tok::Comma).is_some() {
                names.push(self.parse_name("after the comma, write the next name to import")?);
            }
            Some(names)
        } else {
            None
        };
        self.end_of_line("the `use` line")?;
        let end = self.peek().1;
        Ok(UseDecl { module, imports, span: start.to(end) })
    }

    // ------------------------------------------------------------------
    // Blocks & statements (docs/13 §1)
    // ------------------------------------------------------------------

    fn block(&mut self, missing_msg: &str) -> PResult<Block> {
        // The lexer emits Newline at the end of the header line, then the
        // Indent of the body — skip the Newline first (7.0.2 layout).
        self.eat_newline_before_indent();
        let Some(is) = self.eat_tok(&Tok::Indent) else {
            let (_, span) = self.peek();
            self.errors
                .push(Diagnostic::error("E0202", missing_msg, span));
            return Err(());
        };
        let mut stmts = Vec::new();
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            match self.statement() {
                Ok(Some(s)) => stmts.push(s),
                Ok(None) => {}
                Err(()) => self.skip_to_line_end(),
            }
        }
        self.expect_tok(Tok::Dedent, "the end of this block (un-dent)")?;
        let span = is.to(self.peek().1);
        Ok(Block { stmts, span })
    }

    /// Skip one Newline that sits between a header and its body's Indent.
    fn eat_newline_before_indent(&mut self) {
        if self.at_newline() {
            self.bump();
        }
    }

    /// Consume the Newline that ends a logical line (clauses, headers).
    fn end_of_line(&mut self, what: &str) -> PResult<()> {
        if self.at_newline() {
            self.bump();
            Ok(())
        } else if self.at_dedent() || self.at_eof() {
            Ok(())
        } else if self.peek_tok() == Some(&Tok::Comma) {
            // 7.9: a comma is an argument separator inside a call, so a stray
            // one at line end almost always means the head's word-run swallowed
            // the call (`say greet who, "!"` reads as one name run). Say what
            // happened and how to say it — never a bare "end of line" error.
            let (_, span) = self.peek();
            self.errors.push(
                Diagnostic::error(
                    "E0203",
                    "A comma ends this call, but a comma separates arguments inside one.",
                    span,
                )
                .with_note("the call's name and its first argument read as one name — the comma arrived too late to split them")
                .with_fix("wrap the call in parentheses: say (greet who), \"!\""),
            );
            Err(())
        } else {
            let (item, span) = self.peek();
            self.errors.push(
                Diagnostic::error(
                    "E0203",
                    format!("I expected the end of the line here (after {what})."),
                    span,
                )
                .with_note(format!("found: {}", item_text(item))),
            );
            Err(())
        }
    }

    /// Parse one statement. Returns Ok(None) for skipped trivia.
    fn statement(&mut self) -> PResult<Option<Stmt>> {
        let (item, span) = self.peek();
        match item {
            Item::Tok(Tok::Newline) => {
                self.bump();
                Ok(None)
            }
            Item::Tok(Tok::Eof) | Item::Tok(Tok::Dedent) => Ok(None),
            // Doc comments are trivia at M0 (attached to items in a later pass).
            Item::Tok(Tok::Doc(_)) => {
                self.bump();
                Ok(None)
            }
            Item::Kw(Kw::Make) => self.make_stmt().map(Some),
            Item::Kw(Kw::Set) => self.set_stmt().map(Some),
            Item::Kw(Kw::Increase) => self.change_stmt(false).map(Some),
            Item::Kw(Kw::Decrease) => self.change_stmt(true).map(Some),
            Item::Kw(Kw::If) => self.if_stmt().map(Some),
            Item::Kw(Kw::Repeat) => self.repeat_stmt().map(Some),
            Item::Kw(Kw::Stop) => {
                self.bump();
                self.end_of_line("`stop`")?;
                Ok(Some(Stmt::Stop { span }))
            }
            Item::Kw(Kw::Next) => {
                self.bump();
                self.end_of_line("`next`")?;
                Ok(Some(Stmt::Next { span }))
            }
            Item::Kw(Kw::GiveBack) => {
                self.bump();
                let value = self.expr()?;
                self.end_of_line("`give back`")?;
                Ok(Some(Stmt::GiveBack { value, span }))
            }
            Item::Kw(Kw::FailWith) => {
                self.bump();
                let value = self.expr()?;
                self.end_of_line("`fail with`")?;
                Ok(Some(Stmt::FailWith { value, span }))
            }
            Item::Kw(Kw::Attempt) => self.attempt_stmt().map(Some),
            Item::Kw(Kw::CheckThat) => {
                self.bump();
                let expr = self.expr()?;
                self.end_of_line("`check that`")?;
                let full = span.to(expr_span(&expr));
                Ok(Some(Stmt::CheckThat { expr, span: full }))
            }
            // Any expression-shaped start: a flowing call used for effect (`say …`,
            // `ask …`, `bump c`) or a bare name (exprstmt, 7.15).
            _ => {
                let expr = self.expr()?;
                self.end_of_line("this line")?;
                let full = span.to(expr_span(&expr));
                Ok(Some(Stmt::ExprStmt { expr, span: full }))
            }
        }
    }

    /// `make [changing] name equal to expr [of type T]` (7.2 + 7.10's
    /// annotation form, docs/14 G-11).
    fn make_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::Make, "the word `make`")?;
        let mutable = self.eat_kw(Kw::Changing).is_some();
        let name = self.parse_name("`make` must be followed by a name")?;
        self.expect_kw(Kw::EqualTo, "the words `equal to`")?;
        let value = self.expr()?;
        // Optional `of type` annotation (7.10).
        let annotation = if self.peek_kw() == Some(Kw::OfType) {
            self.bump();
            Some(self.parse_type("After `of type`, write the value's type.")?)
        } else {
            None
        };
        self.end_of_line("the `make` line")?;
        let span = start.to(expr_span(&value));
        Ok(Stmt::Make { mutable, name, value, annotation, span })
    }

    /// `set target to expr` (7.2/7.6).
    fn set_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::Set, "the word `set`")?;
        let target = self.parse_target()?;
        self.expect_kw(Kw::To, "the word `to`")?;
        let value = self.expr()?;
        self.end_of_line("the `set` line")?;
        let span = start.to(expr_span(&value));
        Ok(Stmt::Set { target, value, span })
    }

    /// `increase target by expr` / `decrease target by expr` (7.2).
    fn change_stmt(&mut self, decrease: bool) -> PResult<Stmt> {
        let start = if decrease {
            self.expect_kw(Kw::Decrease, "the word `decrease`")?
        } else {
            self.expect_kw(Kw::Increase, "the word `increase`")?
        };
        let target = self.parse_target()?;
        self.expect_kw(Kw::By, "the word `by`")?;
        let value = self.expr()?;
        self.end_of_line("this line")?;
        let span = start.to(expr_span(&value));
        Ok(Stmt::Change { decrease, target, value, span })
    }

    /// `target = name { prep additive }` (7.15): `things at 2`, `score of p`.
    fn parse_target(&mut self) -> PResult<Target> {
        let base = self.parse_name("`set`/`increase` need a name to change")?;
        let mut accessors = Vec::new();
        loop {
            match self.peek_kw() {
                Some(Kw::At) => {
                    self.bump();
                    let index = self.additive()?;
                    let span = expr_span(&index);
                    accessors.push(ast::Accessor::At { index, span });
                }
                Some(Kw::Of) => {
                    self.bump();
                    let field = self.parse_name("`of` must be followed by a field name")?;
                    let span = field.span;
                    accessors.push(ast::Accessor::Of { span, field });
                }
                _ => break,
            }
        }
        let span = base.span;
        Ok(Target { base, accessors, span })
    }

    /// `if expr block {otherwise if expr block} [otherwise block]` (7.4).
    fn if_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::If, "the word `if`")?;
        let mut branches = Vec::new();
        let cond = self.expr()?;
        let then_block = self.block("This `if` needs a body indented under it.")?;
        branches.push((cond, then_block));
        let mut otherwise = None;
        loop {
            match self.peek_kw() {
                Some(Kw::OtherwiseIf) => {
                    self.bump();
                    let cond = self.expr()?;
                    let block = self.block("This `otherwise if` needs a body indented under it.")?;
                    branches.push((cond, block));
                }
                Some(Kw::Otherwise) => {
                    self.bump();
                    let block = self.block("This `otherwise` needs a body indented under it.")?;
                    otherwise = Some(block);
                    break;
                }
                _ => break,
            }
        }
        let end = otherwise
            .as_ref()
            .map(|b| b.span)
            .unwrap_or_else(|| branches.last().unwrap().1.span);
        Ok(Stmt::If { branches, otherwise, span: start.to(end) })
    }

    /// All three loop forms (7.5). The frozen M0 grammar (docs/13 §1):
    /// `count = number "times"` — a *literal* count (G-3: variable counts use
    /// `repeat while` with a mutable counter).
    fn repeat_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::Repeat, "the word `repeat`")?;
        // The count arm must test the *token* stream — `10` is a Tok::Int, and
        // peek_kw() is None for it.
        if let Some(Tok::Int(v)) = self.peek_tok().cloned() {
            let vspan = self.bump().1;
            self.expect_kw(Kw::TimesWord, "the word `times`")?;
            let binding = if self.eat_kw(Kw::Using).is_some() {
                Some(self.parse_name("`using` must be followed by the loop's name")?)
            } else {
                None
            };
            let block = self.block("This `repeat` needs a body indented under it.")?;
            let block_span = block.span;
            return Ok(Stmt::Repeat(Repeat::Count {
                times: (v, vspan),
                binding,
                body: block,
                span: start.to(block_span),
            }));
        }
        match self.peek_kw() {
            Some(Kw::While) => {
                self.bump();
                let cond = self.expr()?;
                let block = self.block("This `repeat while` needs a body indented under it.")?;
                let block_span = block.span;
                Ok(Stmt::Repeat(Repeat::While { cond, body: block, span: start.to(block_span) }))
            }
            Some(Kw::ForEach) => {
                self.bump();
                // M0 pattern = name, or the two-name index form (G-1, docs/13 §1).
                let item = self.parse_name("`for each` must be followed by a name")?;
                let index = if self.eat_tok(&Tok::Comma).is_some() {
                    Some(self.parse_name("after the comma, write the position's name")?)
                } else {
                    None
                };
                self.expect_kw(Kw::In, "the word `in`")?;
                let iter = self.expr()?;
                let block = self.block("This `repeat for each` needs a body indented under it.")?;
                let block_span = block.span;
                Ok(Stmt::Repeat(Repeat::ForEach { item, index, iter, body: block, span: start.to(block_span) }))
            }
            _ => {
                let (item, span) = self.peek();
                self.errors.push(
                    Diagnostic::error(
                        "E0204",
                        "A `repeat` loop needs one of: a number of times, `while`, or `for each`.",
                        span,
                    )
                    .with_fix(
                        "Write one of: repeat 10 times using i | repeat while <condition> | repeat for each item in <list>",
                    )
                    .with_note(format!("found: {}", item_text(item))),
                );
                Err(())
            }
        }
    }

    /// `attempt orexpr [tail]` — ONE production for statement and expression
    /// position (R-20.1). The propagation tail is statement-only (7.15).
    fn attempt_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::Attempt, "the word `attempt`")?;
        let inner = self.orexpr()?;
        let tail = self.attempt_tail(&inner)?;
        let inner_end = expr_span(&inner);
        match &tail {
            None => {
                self.end_of_line("the `attempt` line")?;
                Ok(Stmt::Attempt { expr: Box::new(inner), tail: None, span: start.to(inner_end) })
            }
            Some(AttemptTail::Propagate) => {
                let end = self.peek().1;
                Ok(Stmt::Attempt { expr: Box::new(inner), tail, span: start.to(end) })
            }
            Some(AttemptTail::IfItFails { then_block, otherwise }) => {
                let end = otherwise
                    .as_ref()
                    .map(|b| b.span)
                    .unwrap_or(then_block.span);
                Ok(Stmt::Attempt { expr: Box::new(inner), tail, span: start.to(end) })
            }
            Some(AttemptTail::As { block, otherwise, .. }) => {
                let end = otherwise.as_ref().map(|b| b.span).unwrap_or(block.span);
                Ok(Stmt::Attempt { expr: Box::new(inner), tail, span: start.to(end) })
            }
        }
    }

    /// The attempt tails (7.15): propagation / if-it-fails / as-binding.
    fn attempt_tail(&mut self, inner: &Expr) -> PResult<Option<AttemptTail>> {
        match self.peek_kw() {
            Some(Kw::AndPassTheProblemOn) => {
                self.bump();
                Ok(Some(AttemptTail::Propagate))
            }
            Some(Kw::IfItFails) => {
                self.bump();
                self.expect_kw(Kw::Then, "the word `then`")?;
                let then_block =
                    self.block("After `if it fails then`, indent the failure-handling body.")?;
                let otherwise = if self.eat_kw(Kw::Otherwise).is_some() {
                    Some(self.block("After `otherwise`, indent the success body.")?)
                } else {
                    None
                };
                Ok(Some(AttemptTail::IfItFails { then_block, otherwise }))
            }
            Some(Kw::As) => {
                self.bump();
                let name = self.parse_name("`as` must be followed by a name for the problem")?;
                let block =
                    self.block("After `attempt … as <name>`, indent the failure-handling body.")?;
                let otherwise = if self.eat_kw(Kw::Otherwise).is_some() {
                    Some(self.block("After `otherwise`, indent the success body.")?)
                } else {
                    None
                };
                Ok(Some(AttemptTail::As { name, block, otherwise }))
            }
            _ => {
                // Bare attempt: in statement position this is a discard of the
                // result — sema decides whether the capability rule allows it.
                let _ = inner;
                Ok(None)
            }
        }
    }

    // ------------------------------------------------------------------
    // Expressions (docs/13 §2 precedence ladder)
    // ------------------------------------------------------------------

    fn expr(&mut self) -> PResult<Expr> {
        self.orexpr()
    }

    /// `orexpr = andexpr { "or" andexpr }`
    fn orexpr(&mut self) -> PResult<Expr> {
        let mut left = self.andexpr()?;
        while self.peek_kw() == Some(Kw::Or) {
            let op_span = self.bump().1;
            let right = self.andexpr()?;
            let span = expr_span(&left).to(expr_span(&right));
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span: op_span.to(span),
            };
        }
        Ok(left)
    }

    /// `andexpr = notexpr { "and" notexpr }`
    fn andexpr(&mut self) -> PResult<Expr> {
        let mut left = self.notexpr()?;
        while self.peek_kw() == Some(Kw::And) {
            let op_span = self.bump().1;
            let right = self.notexpr()?;
            let span = expr_span(&left).to(expr_span(&right));
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span: op_span.to(span),
            };
        }
        Ok(left)
    }

    /// `notexpr = ["not"] comparison`
    fn notexpr(&mut self) -> PResult<Expr> {
        if self.peek_kw() == Some(Kw::Not) {
            let span = self.bump().1;
            let inner = self.comparison()?;
            let full = span.to(expr_span(&inner));
            Ok(Expr::Not { inner: Box::new(inner), span: full })
        } else {
            self.comparison()
        }
    }

    /// `comparison = additive [("is" cmpword | cmpword) additive]`
    fn comparison(&mut self) -> PResult<Expr> {
        let left = self.additive()?;
        let op = match self.peek_kw() {
            Some(Kw::CmpEqualTo) => Some(BinOp::Equal),
            Some(Kw::CmpNotEqualTo) => Some(BinOp::NotEqual),
            Some(Kw::CmpGreaterThan) => Some(BinOp::Greater),
            Some(Kw::CmpLessThan) => Some(BinOp::Less),
            Some(Kw::CmpAtLeast) => Some(BinOp::AtLeast),
            Some(Kw::CmpAtMost) => Some(BinOp::AtMost),
            // option comparisons (M0 basic options, 8.5/D-34)
            Some(Kw::IsNothing) => {
                self.bump();
                let span = expr_span(&left);
                return Ok(Expr::Binary {
                    op: BinOp::Equal,
                    left: Box::new(left),
                    right: Box::new(Expr::Nothing { span }),
                    span,
                });
            }
            Some(Kw::IsSomething) => {
                self.bump();
                let span = expr_span(&left);
                return Ok(Expr::Binary {
                    op: BinOp::NotEqual,
                    left: Box::new(left),
                    right: Box::new(Expr::Nothing { span }),
                    span,
                });
            }
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let right = self.additive()?;
            let span = expr_span(&left).to(expr_span(&right));
            Ok(Expr::Binary { op, left: Box::new(left), right: Box::new(right), span })
        } else {
            Ok(left)
        }
    }

    /// `additive = multiplicative { (+|plus) mult | (-|minus) mult }`
    fn additive(&mut self) -> PResult<Expr> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match (self.peek_tok(), self.peek_kw()) {
                (Some(Tok::Plus), _) => Some(BinOp::Add),
                (Some(Tok::Minus), _) => Some(BinOp::Sub),
                (None, Some(Kw::Plus)) => Some(BinOp::Add),
                (None, Some(Kw::Minus)) => Some(BinOp::Sub),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.bump();
                    let right = self.multiplicative()?;
                    let span = expr_span(&left).to(expr_span(&right));
                    left = Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    };
                }
                None => break,
            }
        }
        Ok(left)
    }

    /// `multiplicative = unary { (*|times) u | (/|divided by) u | divided evenly by u | (remainder of|modulo) u }`
    fn multiplicative(&mut self) -> PResult<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match (self.peek_tok(), self.peek_kw()) {
                (Some(Tok::Star), _) => Some(BinOp::Mul),
                (Some(Tok::Slash), _) => Some(BinOp::Div),
                // `times` doubles as the count-loop word (7.5); in expression
                // position after an operand it is multiplication (7.3).
                (None, Some(Kw::TimesWord)) => Some(BinOp::Mul),
                (None, Some(Kw::DividedBy)) => Some(BinOp::Div),
                (None, Some(Kw::DividedEvenlyBy)) => Some(BinOp::DivEvenly),
                (None, Some(Kw::RemainderOf)) => Some(BinOp::Rem),
                (None, Some(Kw::Modulo)) => Some(BinOp::Rem),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.bump();
                    let right = self.unary()?;
                    let span = expr_span(&left).to(expr_span(&right));
                    left = Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    };
                }
                None => break,
            }
        }
        Ok(left)
    }

    /// `unary = "-" primary | primary`
    fn unary(&mut self) -> PResult<Expr> {
        if let Some(s) = self.eat_tok(&Tok::Minus) {
            let inner = self.primary()?;
            let full = s.to(expr_span(&inner));
            Ok(Expr::Neg { inner: Box::new(inner), span: full })
        } else if self.peek_kw() == Some(Kw::RemainderOf) {
            // 5.1/S-1: the leading call form `remainder of total and 2` — the
            // word operator opening an expression, reading as a call. The
            // infix form (`total remainder of 2`) is handled in
            // multiplicative.
            let rem_span = self.peek().1;
            self.bump();
            let left = self.unary()?;
            self.expect_kw(Kw::And, "`remainder of` needs two numbers: remainder of a and b")?;
            let right = self.unary()?;
            let full = rem_span.to(expr_span(&right));
            Ok(Expr::Binary {
                op: BinOp::Rem,
                left: Box::new(left),
                right: Box::new(right),
                span: full,
            })
        } else {
            self.primary()
        }
    }

    /// `primary = literal | name | call | ( expr ) | listlit | maplit | pairlit | attempt`
    fn primary(&mut self) -> PResult<Expr> {
        let (item, span) = self.peek();
        match item {
            Item::Tok(Tok::Int(v)) => {
                let v = *v;
                self.bump();
                Ok(Expr::Int { value: v, span })
            }
            Item::Tok(Tok::Float(v)) => {
                let v = *v;
                self.bump();
                Ok(Expr::Float { value: v, span })
            }
            Item::Tok(Tok::Text(s)) => {
                let raw = s.clone();
                self.bump();
                self.text_expr(raw, span)
            }
            Item::Kw(Kw::True) => {
                self.bump();
                Ok(Expr::Bool { value: true, span })
            }
            Item::Kw(Kw::False) => {
                self.bump();
                Ok(Expr::Bool { value: false, span })
            }
            Item::Kw(Kw::Nothing) => {
                self.bump();
                Ok(Expr::Nothing { span })
            }
            Item::Kw(Kw::AListOf) => self.list_lit(),
            Item::Kw(Kw::AMapFrom) => self.map_lit(),
            Item::Kw(Kw::APairOf) => self.pair_lit(),
            Item::Kw(Kw::Attempt) => {
                self.bump();
                let inner = self.orexpr()?;
                // Expression-position attempt may not take a tail (7.15: tails
                // appear in statement position; `as`/`if it fails` make the
                // attempt statement-shaped).
                let full = span.to(expr_span(&inner));
                Ok(Expr::AttemptExpr { expr: Box::new(inner), span: full })
            }
            Item::Tok(Tok::LParen) => {
                self.bump();
                let inner = self.expr()?;
                let end = self.expect_tok(Tok::RParen, "a closing parenthesis")?;
                Ok(Expr::Group { inner: Box::new(inner), span: span.to(end) })
            }
            Item::Tok(Tok::WordRun(_)) => self.try_name_or_call(),
            // Conversion heads are type words used as call heads: `number from answer` (D-39).
            Item::Kw(Kw::Number)
            | Item::Kw(Kw::Decimal)
            | Item::Kw(Kw::Text)
            | Item::Kw(Kw::Boolean)
            | Item::Kw(Kw::Random) => self.try_call_from_head(),
            // Construction (7.11 structlit): `a player with name "bo" and score 0`.
            // An article followed by a name opens a construction; otherwise the
            // article is itself a one-word name (7.9: `bigger of a and b`).
            Item::Kw(Kw::A) | Item::Kw(Kw::An) => {
                if matches!(self.items.get(self.pos + 1), Some((Item::Tok(Tok::WordRun(_)), _))) {
                    self.struct_lit()
                } else {
                    let name = self.parse_name("a value")?;
                    Ok(Expr::Name { name, span })
                }
            }
            _ => {
                self.errors.push(
                    Diagnostic::error("E0205", "I expected a value here.", span)
                        .with_note(format!("found: {}", item_text(item))),
                );
                Err(())
            }
        }
    }

    /// Build a text expression, parsing `{expr}` interpolation (7.7).
    fn text_expr(&mut self, raw: String, span: Span) -> PResult<Expr> {
        if !raw.contains('{') {
            return Ok(Expr::Text { value: raw, span });
        }
        let mut parts = Vec::new();
        let mut lit = String::new();
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                if !lit.is_empty() {
                    parts.push(InterpPart::Lit(std::mem::take(&mut lit)));
                }
                let mut inner = String::new();
                let mut depth = 1usize;
                loop {
                    match chars.next() {
                        Some('}') => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            inner.push('}');
                        }
                        Some('{') => {
                            depth += 1;
                            inner.push('{');
                        }
                        Some(ch) => inner.push(ch),
                        None => {
                            self.errors.push(Diagnostic::error(
                                "E0206",
                                "This text has a `{` with no matching `}`.",
                                span,
                            ));
                            return Err(());
                        }
                    }
                }
                // Re-lex and parse the interpolation body as an expression.
                let (program, mut errs) = parse(&inner);
                self.errors.append(&mut errs);
                let expr = match program.items.into_iter().next() {
                    Some(AstItem::Stmt(Stmt::ExprStmt { expr, .. })) => expr,
                    Some(_) => {
                        self.errors.push(Diagnostic::error(
                            "E0206",
                            "Inside `{…}` in text, write an expression.",
                            span,
                        ));
                        return Err(());
                    }
                    None => {
                        self.errors.push(Diagnostic::error(
                            "E0206",
                            "This `{…}` in text is empty — write an expression inside.",
                            span,
                        ));
                        return Err(());
                    }
                };
                parts.push(InterpPart::Expr(expr));
            } else if c == '}' {
                self.errors.push(Diagnostic::error(
                    "E0206",
                    "This text has a `}` with no matching `{`.",
                    span,
                ));
                return Err(());
            } else {
                lit.push(c);
            }
        }
        if !lit.is_empty() {
            parts.push(InterpPart::Lit(lit));
        }
        Ok(Expr::Interp { parts, span })
    }

    /// `a list of expr {, expr}` (7.6).
    fn list_lit(&mut self) -> PResult<Expr> {
        let start = self.expect_kw(Kw::AListOf, "the words `a list of`")?;
        let mut elements = vec![self.expr()?];
        while self.eat_tok(&Tok::Comma).is_some() {
            elements.push(self.expr()?);
        }
        let end = expr_span(elements.last().unwrap());
        Ok(Expr::ListLit { elements, span: start.to(end) })
    }

    /// `a map from primary to primary {, primary to primary}` — keys are
    /// primaries (R-5: `home to work` is the pair, never a call).
    fn map_lit(&mut self) -> PResult<Expr> {
        let start = self.expect_kw(Kw::AMapFrom, "the words `a map from`")?;
        let mut entries = Vec::new();
        loop {
            let key = self.primary()?;
            self.expect_kw(Kw::To, "the word `to` (between a map's key and value)")?;
            let value = self.primary()?;
            entries.push((key, value));
            if self.eat_tok(&Tok::Comma).is_none() {
                break;
            }
        }
        let end = expr_span(&entries.last().unwrap().1);
        Ok(Expr::MapLit { entries, span: start.to(end) })
    }

    /// `a pair of expr and expr` (7.6).
    fn pair_lit(&mut self) -> PResult<Expr> {
        let start = self.expect_kw(Kw::APairOf, "the words `a pair of`")?;
        // Pair elements are additive-level: the joining `and` stays the pair's
        // own separator, and boolean operands need parentheses (7.9's rule).
        let first = self.additive()?;
        self.expect_kw(Kw::And, "the word `and` (joining the pair)")?;
        let second = self.additive()?;
        let span = start.to(expr_span(&second));
        Ok(Expr::PairLit { first: Box::new(first), second: Box::new(second), span })
    }

    /// A WordRun is a name; it becomes a call if an argument follows (7.15
    /// `flowcall = name [additive] { prep additive } { "and" additive }`).
    fn try_name_or_call(&mut self) -> PResult<Expr> {
        let callee = self.parse_name("a value or a call")?;
        // no argument tokens follow → plain name
        if !self.starts_argument() {
            let span = callee.span;
            return Ok(Expr::Name { name: callee, span });
        }
        self.finish_call(callee)
    }

    /// A type word used as a call head (`number from answer`, D-39). If no
    /// argument follows, this is not a derivable expression — the type words
    /// are not names (7.0.3). `try_call_from_head` is entered only from
    /// expression position, so an argument is required.
    fn try_call_from_head(&mut self) -> PResult<Expr> {
        let (item, span) = self.peek();
        let callee = match item {
            Item::Kw(k @ (Kw::Number | Kw::Decimal | Kw::Text | Kw::Boolean | Kw::Random)) => {
                let word = kw_text(*k);
                let kspan = span;
                Name { words: vec![word.to_string()], span: kspan }
            }
            _ => unreachable!("try_call_from_head entered on a non-type word"),
        };
        self.bump();
        if !self.starts_argument() {
            self.errors.push(
                Diagnostic::error(
                    "E0205",
                    format!("`{}` needs a value — write it as a conversion: `{} from <text>`.", callee.display(), callee.display()),
                    callee.span,
                )
                .with_fix("For example: number from answer"),
            );
            return Err(());
        }
        self.finish_call(callee)
    }

    /// Construction (7.11 structlit): `a player with name "bo" and score 0` —
    /// the second+ `with` spells `and` (7.11). Requires a user-type name.
    fn struct_lit(&mut self) -> PResult<Expr> {
        let start = self.peek().1;
        self.eat_kw(Kw::A);
        self.eat_kw(Kw::An);
        let name = self.parse_name("`a` must be followed by the structure's name")?;
        let mut fields: Vec<(Name, Expr)> = Vec::new();
        while self.peek_kw() == Some(Kw::With) || self.peek_kw() == Some(Kw::And) {
            let conj = self.peek_kw();
            self.bump();
            if conj == Some(Kw::With) && fields.is_empty() {
                // first `with`
            } else if conj == Some(Kw::With) {
                self.errors.push(Diagnostic::error(
                    "E0201",
                    "Construction fields after the first spell `and`, not `with`.",
                    self.peek().1,
                ));
                return Err(());
            }
            let fname = self.parse_name("this construction field needs a name")?;
            if !self.starts_additive() {
                let (_, span) = self.peek();
                self.errors.push(Diagnostic::error(
                    "E0207",
                    format!("`{} {}` needs a value.", name.display(), fname.display()),
                    span,
                ));
                return Err(());
            }
            let value = self.additive()?;
            fields.push((fname, value));
        }
        let end = fields.last().map(|(_, v)| expr_span(v)).unwrap_or(start);
        Ok(Expr::StructLit { name, fields, span: start.to(end) })
    }

    /// Could an argument follow the call head here? Arguments are either
    /// additive-level values (R-4) or prepositional (`of/at/from/to` + value —
    /// `name of p`, `random from 1 to 6`).
    fn starts_argument(&self) -> bool {
        self.starts_additive()
            || matches!(
                self.peek_kw(),
                Some(Kw::Of | Kw::At | Kw::From | Kw::To)
            )
    }

    /// The unified call grammar (7.15, R-1): first positional arg, prepositional
    /// args, `and`-separated args, then `with`-labeled suffixes.
    fn finish_call(&mut self, callee: Name) -> PResult<Expr> {
        let start = callee.span;
        let first = if self.starts_additive() {
            let e = self.additive()?;
            let span = expr_span(&e);
            Some(Box::new(Arg { expr: Box::new(e), span }))
        } else {
            None
        };
        let mut preps = Vec::new();
        loop {
            let prep = match self.peek_kw() {
                Some(Kw::Of) => Prep::Of,
                Some(Kw::At) => Prep::At,
                Some(Kw::From) => Prep::From,
                Some(Kw::To) => Prep::To,
                _ => break,
            };
            self.bump();
            let e = self.additive()?;
            let span = expr_span(&e);
            preps.push((prep, Arg { expr: Box::new(e), span }));
        }
        let mut and_args = Vec::new();
        // 5.1/S-1: the word operator `remainder of` reads call-shaped —
        // `remainder of total and 2` — so it is parsed as the call form of the
        // arithmetic operator (7.9: word operators have an infix and a call
        // form; this one's infix form is `a remainder of b`).
        if self.peek_kw() == Some(Kw::RemainderOf) {
            self.bump();
            let second = self.additive()?;
            let span = callee.span.to(expr_span(&second));
            return Ok(Expr::Binary {
                op: BinOp::Rem,
                left: Box::new(Expr::Name { name: callee.clone(), span: callee.span }),
                right: Box::new(second),
                span,
            });
        }
        loop {
            // 7.9: the `and` form and the comma form are interchangeable
            // argument separators, and may be mixed (`bigger of a, b`).
            let is_and = self.peek_kw() == Some(Kw::And);
            let is_comma = self.peek_tok() == Some(&Tok::Comma);
            if !is_and && !is_comma {
                break;
            }
            self.bump();
            let e = self.additive()?;
            let span = expr_span(&e);
            and_args.push(Arg { expr: Box::new(e), span });
        }
        let mut with_args = Vec::new();
        while self.peek_kw() == Some(Kw::With) {
            self.bump();
            let label = self.parse_name("`with` must be followed by the argument's name")?;
            if !self.starts_additive() {
                let (_, span) = self.peek();
                self.errors.push(Diagnostic::error(
                    "E0207",
                    format!("`with {}` needs a value after the name.", label.display()),
                    span,
                ));
                return Err(());
            }
            let e = self.additive()?;
            let span = expr_span(&e);
            with_args.push((label, Arg { expr: Box::new(e), span }));
        }
        let end = and_args
            .last()
            .or_else(|| with_args.last().map(|(_, a)| a))
            .or_else(|| preps.last().map(|(_, a)| a))
            .or(first.as_deref())
            .map(|a| a.span)
            .unwrap_or(start);
        self.check_ambiguous_or(&callee)?;
        Ok(Expr::Call(Box::new(CallExpr { callee, first, preps, and_args, with_args, span: start.to(end) })))
    }

    /// 7.9 (R-4): `bigger of a and b or c` is a compile error with a teaching
    /// diagnostic naming both readings — the ambiguity is caught, never
    /// silently misparsed.
    fn check_ambiguous_or(&mut self, callee: &Name) -> PResult<()> {
        if self.peek_kw() != Some(Kw::Or) {
            return Ok(());
        }
        let or_pos = self.peek().1.start;
        let text = self.src;
        let head = text
            .get(callee.span.start..or_pos)
            .unwrap_or("")
            .trim_end()
            .to_string();
        let rest = text.get(or_pos..).unwrap_or("").trim_end().to_string();
        // Split the consumed call text at its last ` and ` so the two readings
        // quote the user's own words (7.9: the diagnostic names both readings).
        let (reading1, reading2) = match head.rfind(" and ") {
            Some(pos) => {
                let lhs = head[..pos].to_string();
                let mid = head[pos + " and ".len()..].to_string();
                (
                    format!("{lhs} and ({mid} {rest})"),
                    format!("({head}) {rest}"),
                )
            }
            None => (format!("({head}) or (...)"), format!("{head} or (...)")),
        };
        let msg = format!(
            "I cannot tell which of two things you mean — did you mean `{reading1}`, or `{reading2}`? Put parentheses around the part you meant."
        );
        self.errors.push(
            Diagnostic::error("E0209", msg, self.peek().1)
                .with_explanation(
                    "Inside a call, `and` separates arguments; an `or` after the arguments is ambiguous. Boolean arguments need parentheses.",
                ),
        );
        Err(())
    }

    /// Could an additive expression start at the current item?
    fn starts_additive(&self) -> bool {
        match self.peek().0 {
            Item::Tok(Tok::Int(_) | Tok::Float(_) | Tok::Text(_) | Tok::LParen | Tok::Minus) => true,
            Item::Kw(
                Kw::True
                | Kw::False
                | Kw::Nothing
                | Kw::AListOf
                | Kw::AMapFrom
                | Kw::APairOf
                | Kw::Attempt
                | Kw::Number
                | Kw::Decimal
                | Kw::Text
                | Kw::Boolean
                | Kw::Random,
            ) => true,
            Item::Tok(Tok::WordRun(_)) => true,
            _ => false,
        }
    }

    /// Parse a Name: a WordRun (one or more words), or the standalone article
    /// `a`/`an` used as a one-word name — §7.9's own example `bigger of a and b`
    /// uses `a` as a variable, so articles are *contextual*: they are articles
    /// in type/construction positions and names everywhere else.
    fn parse_name(&mut self, expecting: &str) -> PResult<Name> {
        match self.peek() {
            (Item::Tok(Tok::WordRun(words)), span) => {
                let words = words.clone();
                let span = span;
                self.bump();
                Ok(Name { words, span })
            }
            (Item::Kw(k @ (Kw::A | Kw::An)), span) => {
                let word = kw_text(*k).to_string();
                let span = span;
                self.bump();
                Ok(Name { words: vec![word], span })
            }
            _ => {
                let (item, span) = self.peek();
                self.errors.push(
                    Diagnostic::error("E0208", format!("I expected a name here ({expecting})."), span)
                        .with_note(format!("found: {}", item_text(item))),
                );
                Err(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ty_span(t: &TypeExpr) -> Span {
    match t {
        TypeExpr::User(n) => n.span,
        // Compound types' inner spans are Box'd; the type starts at the inner
        // head word in practice — but we only need an end anchor here.
        TypeExpr::List(t) | TypeExpr::Map(t, _) | TypeExpr::Pair(t, _) => ty_span_b(t),
        _ => Span::default(),
    }
}

fn ty_span_b(t: &TypeExpr) -> Span {
    ty_span(t)
}

fn expr_span(e: &Expr) -> Span {
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

fn kw_text(kw: Kw) -> &'static str {
    use Kw::*;
    match kw {
        Make => "make",
        Changing => "changing",
        EqualTo => "equal to",
        Set => "set",
        To => "to",
        Increase => "increase",
        Decrease => "decrease",
        By => "by",
        If => "if",
        OtherwiseIf => "otherwise if",
        Otherwise => "otherwise",
        Repeat => "repeat",
        TimesWord => "times",
        Using => "using",
        While => "while",
        ForEach => "for each",
        In => "in",
        Stop => "stop",
        Next => "next",
        Function => "function",
        Takes => "takes",
        Called => "called",
        Returns => "returns",
        GiveBack => "give back",
        FailWith => "fail with",
        CanFail => "can fail",
        Attempt => "attempt",
        AndPassTheProblemOn => "and pass the problem on",
        IfItFails => "if it fails",
        Then => "then",
        As => "as",
        Test => "test",
        CheckThat => "check that",
        Use => "use",
        For => "for",
        Structure => "structure",
        Has => "has",
        OfType => "of type",
        WaitForAllTasks => "wait for all tasks",
        And => "and",
        Or => "or",
        Not => "not",
        Is => "is",
        CmpEqualTo => "is equal to",
        CmpNotEqualTo => "is not equal to",
        CmpGreaterThan => "is greater than",
        CmpLessThan => "is less than",
        CmpAtLeast => "is at least",
        CmpAtMost => "is at most",
        Plus => "plus",
        Minus => "minus",
        DividedBy => "divided by",
        DividedEvenlyBy => "divided evenly by",
        RemainderOf => "remainder of",
        Modulo => "modulo",
        True => "true",
        False => "false",
        Nothing => "nothing",
        AListOf => "a list of",
        AMapFrom => "a map from",
        APairOf => "a pair of",
        Number => "number",
        Decimal => "decimal",
        Text => "text",
        Boolean => "boolean",
        Random => "random",
        IsNothing => "is nothing",
        IsSomething => "is something",
        Of => "of",
        At => "at",
        From => "from",
        With => "with",
        A => "a",
        An => "an",
    }
}

fn item_text(item: &Item) -> String {
    match item {
        Item::Tok(Tok::Newline) => "end of line".into(),
        Item::Tok(Tok::Eof) => "end of file".into(),
        Item::Tok(Tok::Indent) => "an indented block".into(),
        Item::Tok(Tok::Dedent) => "the end of a block".into(),
        Item::Tok(Tok::Int(v)) => format!("the number {v}"),
        Item::Tok(Tok::Float(v)) => format!("the decimal {v}"),
        Item::Tok(Tok::Text(s)) => format!("the text \"{s}\""),
        Item::Tok(Tok::Doc(d)) => format!("the doc comment `{d}`"),
        Item::Tok(Tok::WordRun(w)) => format!("the name \"{}\"", w.join(" ")),
        Item::Tok(t) => format!("the symbol {t:?}"),
        Item::Kw(k) => format!("the word `{}`", kw_text(*k)),
    }
}

// ---------------------------------------------------------------------------
// Tests — derived from the frozen §7.15 grammar via docs/13, using the same
// canonical examples as the lexer tests (no second grammar).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lagom_diagnostics::SourceFile;

    fn parse_ok(src: &str) -> Program {
        let (p, errs) = parse(src);
        assert!(
            errs.is_empty(),
            "expected no diagnostics for:\n{src}\ngot: {:?}",
            errs.iter().map(|e| render_student_for_test(src, e)).collect::<Vec<_>>()
        );
        p
    }

    fn parse_err(src: &str, code: &str) -> Vec<Diagnostic> {
        let (_, errs) = parse(src);
        assert!(
            errs.iter().any(|e| e.code == code),
            "expected diagnostic {code} for:\n{src}\ngot: {:?}",
            errs.iter().map(|e| render_student_for_test(src, e)).collect::<Vec<_>>()
        );
        errs
    }

    /// Render with a dummy source file so failures print like user output.
    fn render_student_for_test(src: &str, d: &Diagnostic) -> String {
        let f = SourceFile::new("test.lagom", src);
        lagom_diagnostics::render_student(&f, d)
    }

    fn first_stmt(p: &Program) -> &Stmt {
        match p.items.first().expect("expected at least one item") {
            AstItem::Stmt(s) => s,
            other => panic!("expected a statement, got {other:?}"),
        }
    }

    fn single_expr(p: &Program) -> &Expr {
        match first_stmt(p) {
            Stmt::ExprStmt { expr, .. } => expr,
            other => panic!("expected an expression statement, got {other:?}"),
        }
    }

    // ----- make / set / increase / decrease (7.2) -----

    #[test]
    fn make_immutable_and_mutable() {
        let p = parse_ok("make score equal to 10\nmake changing total equal to 0");
        assert_eq!(p.items.len(), 2);
        match first_stmt(&p) {
            Stmt::Make { mutable, name, value, .. } => {
                assert!(!mutable);
                assert_eq!(name.display(), "score");
                assert!(matches!(value, Expr::Int { value: 10, .. }));
            }
            other => panic!("{other:?}"),
        }
        match p.items.last().unwrap() {
            AstItem::Stmt(Stmt::Make { mutable, name, .. }) => {
                assert!(*mutable);
                assert_eq!(name.display(), "total");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn set_with_index_and_field_targets() {
        let p = parse_ok("set things at 2 to \"x\"\nset score of p to 5");
        assert_eq!(p.items.len(), 2);
        match first_stmt(&p) {
            Stmt::Set { target, .. } => {
                assert_eq!(target.base.display(), "things");
                assert!(matches!(target.accessors[0], ast::Accessor::At { .. }));
            }
            other => panic!("{other:?}"),
        }
        match p.items.last().unwrap() {
            AstItem::Stmt(Stmt::Set { target, .. }) => {
                assert_eq!(target.base.display(), "score");
                assert!(matches!(target.accessors[0], ast::Accessor::Of { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn increase_and_decrease_valid() {
        let p = parse_ok("increase score by 1\ndecrease lives by 2");
        match p.items.last().unwrap() {
            AstItem::Stmt(Stmt::Change { decrease: true, target, .. }) => {
                assert_eq!(target.base.display(), "lives");
            }
            other => panic!("{other:?}"),
        }
        let _ = first_stmt(&p);
    }

    // ----- say / ask (7.7, 7.9 call forms) -----

    #[test]
    fn say_plain_and_interpolated() {
        parse_ok("say \"Hello, world!\"");
        parse_ok("say \"Hello, {name}! You have {count} points.\"");
    }

    #[test]
    fn ask_positional_call() {
        let p = parse_ok("make answer equal to ask \"What is your name?\"");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                assert_eq!(c.callee.display(), "ask");
                assert!(c.first.is_some());
                assert!(c.and_args.is_empty());
                assert!(c.preps.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- the R-4 rule: and is an argument separator; ambiguous or is an error -----

    #[test]
    fn divide_two_args() {
        let p = parse_ok("attempt divide 10 and 0");
        match first_stmt(&p) {
            Stmt::Attempt { expr, .. } => match expr.as_ref() {
                Expr::Call(c) => {
                    assert_eq!(c.callee.display(), "divide");
                    assert!(c.first.is_some());
                    assert_eq!(c.and_args.len(), 1);
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn send_to_and_download_to_prepositional() {
        let p = parse_ok("send \"hello\" to messages");
        match single_expr(&p) {
            Expr::Call(c) => {
                assert_eq!(c.callee.display(), "send");
                assert_eq!(c.preps.len(), 1);
                assert_eq!(c.preps[0].0, Prep::To);
            }
            other => panic!("{other:?}"),
        }
        let p = parse_ok("download \"a\" to \"a.file\"");
        match single_expr(&p) {
            Expr::Call(c) => assert_eq!(c.preps[0].0, Prep::To),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bigger_of_a_and_b() {
        let p = parse_ok("make larger equal to bigger of a and b");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                assert_eq!(c.callee.display(), "bigger");
                assert_eq!(c.preps.len(), 1);
                assert_eq!(c.preps[0].0, Prep::Of);
                assert_eq!(c.and_args.len(), 1);
            }
            other => panic!("{other:?}"),
        }
    }

    /// 7.9: `bigger of a and b or c` is a **parse error with a teaching
    /// diagnostic** — the ambiguity is caught, never silently misparsed.
    #[test]
    fn and_ambiguity_is_error_with_teaching_diagnostic() {
        let errs = parse_err("make x equal to bigger of a and b or c", "E0209");
        let d = errs.iter().find(|e| e.code == "E0209").unwrap();
        // The diagnostic must name both readings (7.9).
        assert!(
            d.message.contains("bigger of a and") && d.message.contains("b or c"),
            "diagnostic must name both readings, got: {}",
            d.message
        );
    }

    // ----- remainder of (5.1/S-1): infix and call forms -----

    /// The leading call form from the spec's own example: `remainder of total
    /// and 2` parses as the Rem operator over its two operands.
    #[test]
    fn remainder_of_leading_call_form() {
        let p = parse_ok("make rest equal to remainder of total and 2");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Binary { op: BinOp::Rem, left, right, .. }, .. } => {
                assert!(matches!(left.as_ref(), Expr::Name { .. }));
                assert!(matches!(right.as_ref(), Expr::Int { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    /// The infix form keeps working: `7 remainder of 2`.
    #[test]
    fn remainder_of_infix_form() {
        let p = parse_ok("make r equal to 7 remainder of 2");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Binary { op: BinOp::Rem, .. }, .. } => {}
            other => panic!("{other:?}"),
        }
    }

    /// A call taking `remainder of` as a later argument keeps the operator
    /// reading: `text from remainder of a and b` is text(rem(a, b)) — the
    /// `from` prepositional slot carries the operator expression.
    #[test]
    fn remainder_of_inside_call_argument() {
        let p = parse_ok("make s equal to text from remainder of a and b");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                assert_eq!(c.callee.display(), "text");
                assert_eq!(c.preps.len(), 1, "the `from` prep carries the argument");
                assert!(matches!(
                    c.preps[0].1.expr.as_ref(),
                    Expr::Binary { op: BinOp::Rem, .. }
                ));
            }
            other => panic!("{other:?}"),
        }
    }

    /// 7.9: a stray comma at line end — the word-run swallowed the call — is
    /// a teaching diagnostic naming the fix, never a bare end-of-line error.
    #[test]
    fn stray_comma_after_word_run_is_teaching_diagnostic() {
        let errs = parse_err("say greet who, \"!\"", "E0203");
        let d = errs.iter().find(|e| e.code == "E0203").unwrap();
        assert!(
            d.message.contains("comma") && d.fix.as_deref().unwrap_or("").contains("("),
            "diagnostic must explain the swallowed call, got: {:?} / fix: {:?}",
            d.message,
            d.fix
        );
    }

    /// Boolean arguments need parentheses: `say flag and other` parses as the
    /// boolean `and` (the call consumed only `flag`); inside a call, a bare
    /// `or` after `and`-args is the E0209 spelling.
    #[test]
    fn boolean_and_in_call_greedy_at_parser_level() {
        // `say ready and other` lexes as WordRun("say ready"), Kw(And),
        // WordRun("other"): at parse level the andexpr ladder sees one binary
        // `and` between two names — splitting `say ready` into callee+arg is
        // sema's 7.0.3 greedy job (the frozen division of labor).
        let (p, errs) = parse("say ready and other");
        assert!(errs.is_empty());
        match single_expr(&p) {
            Expr::Binary { op: BinOp::And, .. } => {}
            other => panic!("{other:?}"),
        }
    }

    // ----- comparison words (7.3) -----

    #[test]
    fn comparisons_all_forms() {
        for src in [
            "make a equal to x is equal to y",
            "make b equal to x is not equal to y",
            "make c equal to x is greater than y",
            "make d equal to x is less than y",
            "make e equal to x is at least y",
            "make f equal to x is at most y",
        ] {
            let p = parse_ok(src);
            assert!(matches!(first_stmt(&p), Stmt::Make { .. }), "{src}");
        }
    }

    #[test]
    fn option_comparisons() {
        parse_ok("make found equal to first of scores is nothing");
        parse_ok("make got equal to top score is something");
    }

    // ----- arithmetic ladder incl. word operators (7.3) -----

    #[test]
    fn arithmetic_word_and_symbol_forms() {
        parse_ok("make t equal to total divided evenly by size of scores");
        parse_ok("make r equal to 10 modulo 3");
        parse_ok("make m equal to 2 times 3 plus 4");
        parse_ok("make n equal to 2 * 3 + 4");
        parse_ok("make d equal to a divided by b");
        let p = parse_ok("make neg equal to -5 plus 1");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Binary { op: BinOp::Add, .. }, .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parens_override_precedence() {
        let p = parse_ok("make x equal to (2 plus 3) times 4");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Binary { op: BinOp::Mul, left, .. }, .. } => {
                assert!(matches!(left.as_ref(), Expr::Group { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- collections (7.6) -----

    #[test]
    fn list_literal() {
        let p = parse_ok("make things equal to a list of 1, 2, 3");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::ListLit { elements, .. }, .. } => {
                assert_eq!(elements.len(), 3);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn map_literal_keys_are_primaries() {
        let p = parse_ok("make ages equal to a map from \"ana\" to 11, \"bo\" to 12");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::MapLit { entries, .. }, .. } => {
                assert_eq!(entries.len(), 2);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn pair_literal() {
        let p = parse_ok("make pt equal to a pair of 3 and 4");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::PairLit { .. }, .. } => {}
            other => panic!("{other:?}"),
        }
    }

    // ----- control flow: if (7.4) -----

    #[test]
    fn if_otherwise_if_otherwise() {
        let src = "\
if score is at least 90
    say \"A\"
otherwise if score is at least 80
    say \"B\"
otherwise
    say \"Keep trying\"
";
        let p = parse_ok(src);
        match first_stmt(&p) {
            Stmt::If { branches, otherwise, .. } => {
                assert_eq!(branches.len(), 2);
                assert!(otherwise.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- all three loop forms (7.5) -----

    #[test]
    fn repeat_count_times_using() {
        let src = "repeat 10 times using i\n    say i\n";
        let p = parse_ok(src);
        match first_stmt(&p) {
            Stmt::Repeat(Repeat::Count { times: (10, _), binding: Some(b), .. }) => {
                assert_eq!(b.display(), "i");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn repeat_while() {
        let src = "repeat while lives is greater than 0\n    increase lives by -1\n";
        parse_ok(src);
    }

    #[test]
    fn repeat_for_each_and_index_form() {
        let src = "repeat for each score in scores\n    say score\n";
        parse_ok(src);
        let src = "repeat for each word, position in words\n    say position\n";
        parse_ok(src);
    }

    #[test]
    fn stop_and_next() {
        let src = "repeat 10 times\n    if i is equal to 3\n        next\n    if i is greater than 8\n        stop\n";
        parse_ok(src);
        // and `times` still multiplies in expression position (7.3)
        parse_ok("make area equal to width times height");
    }

    // ----- functions (7.8) -----

    #[test]
    fn function_takes_returns_body() {
        let src = "\
function calculate average
    takes a list of numbers called scores
    returns a number
    make total equal to 0
    repeat for each score in scores
        increase total by score
    give back total divided evenly by size of scores
";
        let p = parse_ok(src);
        match p.items.first().unwrap() {
            AstItem::Function(f) => {
                assert_eq!(f.name.display(), "calculate average");
                assert_eq!(f.params.len(), 1);
                assert_eq!(f.params[0].name.display(), "scores");
                assert!(matches!(f.params[0].ty, TypeExpr::List(_)));
                assert!(f.returns.is_some());
                assert!(!f.can_fail);
            }
            other => panic!("{other:?}"),
        }
    }

    /// 7.8: the beginner's signature — `takes number of correct answers`
    /// (type word, filler `of`, name `correct answers`).
    #[test]
    fn takes_filler_of_form() {
        let src = "\
function calculate score
    takes number of correct answers
    takes number of total questions
    returns a number
    give back correct answers divided by total questions
";
        let p = parse_ok(src);
        match p.items.first().unwrap() {
            AstItem::Function(f) => {
                assert_eq!(f.params[0].name.display(), "correct answers");
                assert!(matches!(f.params[0].ty, TypeExpr::Number));
                assert_eq!(f.params[1].name.display(), "total questions");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn function_can_fail_and_greet() {
        let src = "\
function greet
    takes text called name
    say \"Hello, {name}!\"
";
        let p = parse_ok(src);
        match p.items.first().unwrap() {
            AstItem::Function(f) => {
                assert_eq!(f.name.display(), "greet");
                assert_eq!(f.params[0].name.display(), "name");
                assert!(matches!(f.params[0].ty, TypeExpr::Text));
            }
            other => panic!("{other:?}"),
        }
        let src = "\
function load config
    can fail
    give back nothing
";
        let _ = parse_ok(src);
        let src3 = "function risky\n    can fail\n    say \"x\"\n";
        let p = parse_ok(src3);
        match p.items.first().unwrap() {
            AstItem::Function(f) => assert!(f.can_fail),
            other => panic!("{other:?}"),
        }
    }

    // ----- structs (7.11) -----

    #[test]
    fn structure_and_construction() {
        let src = "\
structure player
    has name of type text
    has score of type number

make p equal to a player with name \"bo\" and score 0
say name of p
increase score of p by 10
";
        let p = parse_ok(src);
        match p.items.first().unwrap() {
            AstItem::Structure(s) => {
                assert_eq!(s.name.display(), "player");
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name.display(), "name");
                assert!(matches!(s.fields[0].ty, TypeExpr::Text));
                assert!(matches!(s.fields[1].ty, TypeExpr::Number));
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- modules (7.13) -----

    #[test]
    fn use_decl_forms() {
        let p = parse_ok("use math for square root, floor");
        match p.items.first().unwrap() {
            AstItem::Use(u) => {
                assert_eq!(u.module.display(), "math");
                assert_eq!(u.imports.as_ref().unwrap().len(), 2);
            }
            other => panic!("{other:?}"),
        }
        let p = parse_ok("use drawing");
        match p.items.first().unwrap() {
            AstItem::Use(u) => assert!(u.imports.is_none()),
            other => panic!("{other:?}"),
        }
    }

    // ----- attempt & tails (13.1, R-20.1) -----

    #[test]
    fn attempt_if_it_fails_otherwise() {
        // 13.1's exact form: the tail is on the same line as the attempt.
        let src = "\
attempt divide 10 and 0 if it fails then
    say problem
otherwise
    say result
";
        let p = parse_ok(src);
        match first_stmt(&p) {
            Stmt::Attempt { tail: Some(AttemptTail::IfItFails { then_block, otherwise }), .. } => {
                assert_eq!(then_block.stmts.len(), 1);
                assert!(otherwise.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn attempt_as_binding() {
        // 13.1's `as` form, on one line: `attempt load config as problem`.
        let src = "\
attempt load config as problem
    say \"failed: {problem}\"
";
        let p = parse_ok(src);
        match first_stmt(&p) {
            Stmt::Attempt { tail: Some(AttemptTail::As { name, .. }), .. } => {
                assert_eq!(name.display(), "problem");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn attempt_propagate_phrase_token() {
        let p = parse_ok("attempt divide 10 and 0 and pass the problem on");
        match first_stmt(&p) {
            Stmt::Attempt { tail: Some(AttemptTail::Propagate), .. } => {}
            other => panic!("{other:?}"),
        }
    }

    /// Expression-position attempt has no tail (7.15): `(attempt divide 10 and 0) plus 1`
    /// must parse, with the attempt as the left operand.
    #[test]
    fn attempt_as_expression_no_tail() {
        let p = parse_ok("make x equal to (attempt divide 10 and 0) plus 1");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Binary { op: BinOp::Add, left, .. }, .. } => {
                assert!(matches!(left.as_ref(), Expr::Group { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- tests & check that (5.1) -----

    #[test]
    fn test_block_with_check_that() {
        let src = "\
test \"adds numbers\"
    make sum equal to 2 plus 2
    check that sum is equal to 4
";
        let p = parse_ok(src);
        match p.items.first().unwrap() {
            AstItem::Test(t) => {
                assert_eq!(t.name, "adds numbers");
                assert_eq!(t.body.stmts.len(), 2);
                assert!(matches!(t.body.stmts[1], Stmt::CheckThat { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- docs comments & text features (7.7) -----

    #[test]
    fn doc_comment_attaches_as_item() {
        // Doc comments are skipped as trivia at M0 (attachment is a later pass).
        let (p, errs) = parse("## Greets one person by name.\nfunction greet\n    takes text called name\n    say \"hi\"\n");
        assert!(errs.is_empty());
        assert_eq!(p.items.len(), 1); // the function (the doc line is trivia)
        let _ = &p;
    }

    #[test]
    fn multiline_string_in_make() {
        // G-5: an open string literal continues on the next line regardless of
        // indentation (lexer rule); the parser just sees the joined text.
        let src = "make poem equal to \"line one\nline two\"\nsay poem\n";
        parse_ok(src);
    }

    // ----- conversion calls (D-39, docs/13 S-7) -----

    #[test]
    fn number_from_text_call() {
        let src = "make guess equal to number from answer";
        let p = parse_ok(src);
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                assert_eq!(c.callee.display(), "number");
                assert_eq!(c.preps.len(), 1);
                assert_eq!(c.preps[0].0, Prep::From);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn random_from_to_call() {
        let p = parse_ok("make roll equal to random from 1 to 6");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                assert_eq!(c.callee.display(), "random");
                assert_eq!(c.preps.len(), 2);
                assert_eq!(c.preps[0].0, Prep::From);
                assert_eq!(c.preps[1].0, Prep::To);
            }
            other => panic!("{other:?}"),
        }
    }

    // ----- error recovery & diagnostics for every major production -----

    #[test]
    fn missing_equal_to_in_make_is_diagnostic_not_panic() {
        let errs = parse_err("make score 10", "E0201");
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn set_needs_a_name() {
        let _ = parse_err("set to 5", "E0208");
    }

    #[test]
    fn if_without_body_is_diagnostic() {
        let errs = parse_err("if x is greater than 3\nsay \"a\"", "E0202");
        assert!(!errs.is_empty());
    }

    #[test]
    fn bad_repeat_form_is_teaching_diagnostic() {
        let errs = parse_err("repeat sometimes\n    say 1", "E0204");
        assert!(errs[0].fix.is_some() || errs[0].message.contains("repeat"));
    }

    #[test]
    fn unterminated_interp_is_diagnostic() {
        let _ = parse_err("say \"oops {name\"", "E0206");
    }

    #[test]
    fn structure_without_indent_is_diagnostic() {
        let errs = parse_err("structure player\nsay 1", "E0202");
        assert!(!errs.is_empty());
    }

    #[test]
    fn field_without_has_is_diagnostic() {
        let src = "structure player\n    name of type text\n";
        let _ = parse_err(src, "E0201");
    }

    #[test]
    fn takes_needs_a_type() {
        // `takes called x` has no type; smallest derivable shape gets a
        // diagnostic, not a panic. (E0208: the type position needs a name if a
        // usertype was meant.)
        let src = "function f\n    takes called x\n    say 1\n";
        let _ = parse_err(src, "E0208");
    }

    #[test]
    fn function_without_body_is_diagnostic() {
        let errs = parse_err("function greet\ntakes text called name\nsay 1", "E0202");
        assert!(!errs.is_empty());
    }

    #[test]
    fn test_needs_quoted_name() {
        let _ = parse_err("test adds\n    check that 1 is equal to 1", "E0201");
    }

    #[test]
    fn recovery_keeps_parsing_following_items() {
        // One bad line must not hide the good statement after it.
        let (p, errs) = parse("make bad ==== 3\nmake good equal to 5\n");
        assert!(!errs.is_empty());
        assert!(
            p.items.iter().any(|i| matches!(i, AstItem::Stmt(Stmt::Make { name, .. }) if name.display() == "good")),
            "the good statement after the bad one must still parse"
        );
    }

    #[test]
    fn spans_are_nonzero_and_cover_the_make_line() {
        let src = "make score equal to 10\n";
        let p = parse_ok(src);
        match first_stmt(&p) {
            Stmt::Make { span, value, .. } => {
                let f = SourceFile::new("t", src);
                let (l1, _) = f.line_col(span.start);
                let (l2, _) = f.line_col(span.end.min(src.len() - 1));
                assert_eq!((l1, l2), (1, 1), "span must stay on the make line");
                assert!(matches!(value, Expr::Int { .. }));
            }
            other => panic!("{other:?}"),
        }
    }
}
