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
    Arg, AttemptTail, BinOp, Block, CallExpr, ClassDecl, Expr, FieldDecl, FunctionDecl, InterfaceDecl,
    InterpPart, KindDecl, LambdaBody, Name, Param, Pattern, PatternLiteral, Prep, Program, Repeat,
    Stmt, StructureDecl, Target, TestDecl, TypeExpr, UseDecl, VariantDecl,
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
        closing_dedent: false,
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
    /// Set while a layout block's closing Dedent is being consumed (block
    /// lambdas: 11.1). A `gives back` body statement eats its own trailing
    /// newline, so after the Dedent there is no Newline left for the enclosing
    /// statement's `end_of_line` — it may legitimately end at the Dedent.
    closing_dedent: bool,
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
                Item::Kw(Kw::Class) => match self.class_decl() {
                    Ok(c) => items.push(AstItem::Class(c)),
                    Err(()) => self.skip_decl(),
                },
                Item::Kw(Kw::Interface) => match self.interface_decl() {
                    Ok(i) => items.push(AstItem::Interface(i)),
                    Err(()) => self.skip_decl(),
                },
                Item::Kw(Kw::Kind) => match self.kind_decl() {
                    Ok(k) => items.push(AstItem::Kind(k)),
                    Err(()) => self.skip_decl(),
                },
                // `a type called score is a number` (8.4): the alias statement.
                Item::Kw(Kw::A)
                    if matches!(self.items.get(self.pos + 1), Some((Item::Tok(Tok::WordRun(w)), _)) if w == &["type"]) =>
                {
                    match self.type_alias_decl() {
                        Ok(a) => items.push(AstItem::TypeAlias(a)),
                        Err(()) => self.skip_to_line_end(),
                    }
                }
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
                self.parse_option_suffix(TypeExpr::Number)
            }
            Some(Kw::Decimal) => {
                self.bump();
                self.parse_option_suffix(TypeExpr::Decimal)
            }
            Some(Kw::Text) => {
                self.bump();
                self.parse_option_suffix(TypeExpr::Text)
            }
            Some(Kw::Boolean) => {
                self.bump();
                self.parse_option_suffix(TypeExpr::Boolean)
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
            // 14.3: `a channel of T` — the typed FIFO channel.
            Some(Kw::AChannelOf) => {
                self.bump();
                let elem = self.parse_type("After `a channel of`, write the message type.")?;
                Ok(TypeExpr::Channel(Box::new(elem)))
            }
            Some(Kw::APairOf) => {
                self.bump();
                let a = self.parse_type("`a pair of` needs the first type.")?;
                self.expect_kw(Kw::And, "the word `and` (joining the pair's types)")?;
                let b = self.parse_type("After `and`, write the second type.")?;
                Ok(TypeExpr::Pair(Box::new(a), Box::new(b)))
            }
            _ => {
                // The type parameter (12.1/12.2): `takes anything called x`
                // and `takes some type called x` are the frozen word spellings
                // of one concept. Both arrive as WordRuns (no reserved Kw), so
                // recognize them before the User-name fallback — otherwise
                // `some type called items` is eaten as a two-word name.
                let is_type_param = matches!(self.peek(), (Item::Tok(Tok::WordRun(w)), _)
                    if w == ["anything"].as_slice()
                        || w == ["some", "type"].as_slice()
                        || (w.last().map(|s| s.as_str()) == Some("that")
                            && match &w[..w.len() - 1] {
                                [s] if s == "anything" => true,
                                [s, t] if s == "some" && t == "type" => true,
                                _ => false,
                            }));
                if is_type_param {
                    // One bump: the WordRun already carries both words of
                    // `some type` (word runs break only at keywords).
                    // 12.3's constraint suffix: `that does <interface>` —
                    // `that` is no keyword either, so it rides the same run
                    // (`anything that does …`); split it back out first so
                    // the suffix parser sees a clean `that` run.
                    let trailing_that = matches!(&self.peek().0,
                        Item::Tok(Tok::WordRun(w)) if w.last().map(|s| s.as_str()) == Some("that"));
                    let run_span = self.peek().1;
                    self.bump();
                    if trailing_that {
                        self.items
                            .insert(self.pos, (Item::Tok(Tok::WordRun(vec!["that".to_string()])), run_span));
                    }
                    let iface = self.parse_constraint_suffix()?;
                    // The option tail composes with type parameters too
                    // (`returns some type?` — the option of whatever the
                    // call site's element type is).
                    return self.parse_option_suffix(TypeExpr::TypeParam { iface });
                }
                // usertype: any name word run
                match self.peek_tok() {
                    Some(Tok::WordRun(_)) => {
                        let name = self.parse_name(expecting)?;
                        let ty = TypeExpr::User(name);
                        self.parse_option_suffix(ty)
                    }
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

    /// 12.3's constraint suffix on a type parameter: `that does <interface>`
    /// (optional). `that` and `does` arrive as WordRuns (`does` is a keyword
    /// only inside class headers' `does` lists — here it follows a plain
    /// word), so the suffix is matched by word shape: run "that", keyword
    /// `does`, then the interface's name run.
    fn parse_constraint_suffix(&mut self) -> PResult<Option<(String, Span)>> {
        let at = self.peek().1;
        let is_that = matches!(self.peek(), (Item::Tok(Tok::WordRun(w)), _)
            if w == ["that"].as_slice());
        if !is_that {
            return Ok(None);
        }
        self.bump();
        if !matches!(self.peek_kw(), Some(Kw::Does)) {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0201",
                "I expected the word `does` here (a constraint reads `that does <interface>`).",
                span,
            ));
            return Err(());
        }
        self.bump();
        let name = self.parse_name("`that does` must be followed by the interface's name")?;
        let _ = at;
        Ok(Some((name.display(), at.to(name.span))))
    }

    /// The option-type suffixes (8.5, R-18): after a type, `?` or the word
    /// form `or nothing` spells the same optional type. Applied to atomic
    /// heads (number?, text?) and user types (shape? / a shape or nothing);
    /// the compound heads (`a list of …`) carry their own option tail inside
    /// their element type.
    fn parse_option_suffix(&mut self, ty: TypeExpr) -> PResult<TypeExpr> {
        if self.eat_tok(&Tok::QuestionMark).is_some() {
            return Ok(TypeExpr::OptionT(Box::new(ty)));
        }
        if self.peek_kw() == Some(Kw::OrNothing) {
            self.bump();
            return Ok(TypeExpr::OptionT(Box::new(ty)));
        }
        Ok(ty)
    }

    // ------------------------------------------------------------------
    // Declarations (7.8 clauses, 7.11 structures, 5.1 tests, 7.13 use)
    // ------------------------------------------------------------------

    /// `a type called <name> is a <type>` (8.4, M1–M2): one line, no block.
    /// The name is an ordinary name (multi-word allowed, like any name); the
    /// type is the full type grammar (aliases of aliases compose).
    fn type_alias_decl(&mut self) -> PResult<ast::TypeAliasDecl> {
        let start = self.expect_kw(Kw::A, "the word `a` (as in `a type called score is a number`)")?;
        // The `type` head word: not a keyword, so it arrived as a one-word
        // name-run — take it directly.
        match self.peek() {
            (Item::Tok(Tok::WordRun(w)), span) if w == &["type"] => {
                self.bump();
                let _ = span;
            }
            _ => {
                let (item, span) = self.peek();
                self.errors.push(
                    Diagnostic::error(
                        "E0201",
                        "I expected the word `type` here (as in `a type called score is a number`).",
                        span,
                    )
                    .with_note(format!("found: {}", item_text(item))),
                );
                return Err(());
            }
        }
        self.expect_kw(Kw::Called, "the word `called` (between `type` and the new type's name)")?;
        let name = self.parse_name("`called` must be followed by the new type's name")?;
        self.expect_kw(Kw::IsA, "the words `is a` (between the new type's name and its real type)")?;
        self.splice_absorbed_type_article();
        let ty = self.parse_type("After `is a`, write the type this new name stands for (like `a number`).")?;
        let span = start.to(ty_span(&ty));
        self.end_of_line("the type alias line")?;
        Ok(ast::TypeAliasDecl { name, ty, span })
    }

    /// The `is a` phrase absorbs the article of a compound type head:
    /// `is a list of number` lexes as IsA, `"list"`, `of` (likewise map/pair).
    /// Splice the two tokens back into the single `a list of` phrase token so
    /// `parse_type` sees the ordinary grammar. Parser-local — the frozen
    /// phrase set is untouched (docs/13 §2).
    fn splice_absorbed_type_article(&mut self) {
        let merged = match self.items.get(self.pos) {
            Some((Item::Tok(Tok::WordRun(w)), _)) if w == &["list"] => Some(Kw::AListOf),
            Some((Item::Tok(Tok::WordRun(w)), _)) if w == &["map"] => Some(Kw::AMapFrom),
            Some((Item::Tok(Tok::WordRun(w)), _)) if w == &["pair"] => Some(Kw::APairOf),
            _ => None,
        };
        let Some(kw) = merged else { return };
        // The head's companion preposition must follow (`list of`,
        // `map from`, `pair of`) — anything else is not a type head.
        let companion = matches!(self.items.get(self.pos + 1).map(|(i, _)| i), Some(Item::Kw(Kw::Of | Kw::From)));
        let correct = match kw {
            Kw::AMapFrom => matches!(self.items.get(self.pos + 1).map(|(i, _)| i), Some(Item::Kw(Kw::From))),
            _ => matches!(self.items.get(self.pos + 1).map(|(i, _)| i), Some(Item::Kw(Kw::Of))),
        };
        if !companion || !correct {
            return;
        }
        let start = self.items[self.pos].1;
        let end = self.items[self.pos + 1].1;
        self.items.splice(self.pos..self.pos + 2, [(Item::Kw(kw), start.to(end))]);
    }

    /// `function name {clause} block` (7.8, M0 clauses: takes/returns/can fail).
    /// Clause order is fixed (7.8: trivially parseable — that is the point).
    fn function_decl(&mut self) -> PResult<FunctionDecl> {
        let start = self.expect_kw(Kw::Function, "the word `function`")?;
        let name = self.parse_name("`function` must be followed by a name")?;
        // 10.3: the receiver word is the construction's first parameter — a
        // user function may not collide with the mangled construction name.
        if name.words.len() == 2 && name.words[0] == "construction" {
            self.errors.push(Diagnostic::error(
                "E0201",
                "`construction` starts a class's named constructor — a function cannot use it as its own name.",
                name.span,
            ));
            return Err(());
        }
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

    /// `interface name ⏎ INDENT { can <name> [body] } DEDENT` (10.6). A `can`
    /// line without a body is a requirement; with a body it is a default
    /// implementation (10.6: "default methods included via `can` bodies"),
    /// synthesized as a receiver-first function of the interface's own name —
    /// sema re-targets copies to the conforming class.
    fn interface_decl(&mut self) -> PResult<InterfaceDecl> {
        let start = self.expect_kw(Kw::Interface, "the word `interface`")?;
        let name = self.parse_name("`interface` must be followed by the interface's name")?;
        self.end_of_line("the interface header")?;
        let Some(_is) = self.eat_tok(&Tok::Indent) else {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0202",
                "This interface needs its methods indented under its header.",
                span,
            ));
            return Err(());
        };
        let mut methods: Vec<(Name, Option<FunctionDecl>)> = Vec::new();
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            match self.peek_kw() {
                Some(Kw::Can) => {
                    let mstart = self.peek().1;
                    self.bump();
                    let mut mname = self.parse_name("`can` must be followed by the method's name")?;
                    // Same `to`-tail rule as class methods (§70's `can compare to`).
                    if self.peek_kw() == Some(Kw::To) {
                        let to_span = self.bump().1;
                        mname.words.push("to".to_string());
                        mname.span = mname.span.to(to_span);
                    }
                    self.end_of_line("the method header")?;
                    // The body is optional: absent = requirement, present =
                    // default (same block layout as a class method's).
                    self.eat_newline_before_indent();
                    let Some(block_open) = self.eat_tok(&Tok::Indent) else {
                        methods.push((mname, None));
                        continue;
                    };
                    let mut params = vec![Param {
                        name: Name { words: vec!["myself".to_string()], span: mname.span },
                        ty: TypeExpr::User(name.clone()),
                        span: mname.span,
                    }];
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
                                let ty =
                                    self.parse_type("After `returns`, write a type (like `a number`).")?;
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
                    let body = self.block_after_open(block_open)?;
                    let full_name = format!("method {} {}", name.display(), mname.display());
                    let fd = FunctionDecl {
                        name: Name { words: vec![full_name], span: mname.span },
                        params,
                        returns,
                        can_fail,
                        body,
                        span: mstart.to(self.peek().1),
                    };
                    methods.push((mname, Some(fd)));
                }
                _ => {
                    let (item, span) = self.peek();
                    self.errors.push(
                        Diagnostic::error("E0201", "I expected `can` here.", span)
                            .with_explanation(
                                "An interface's body is `can <name>` method lines — with a body for a default implementation.",
                            )
                            .with_note(format!("found: {}", item_text(item))),
                    );
                    return Err(());
                }
            }
        }
        self.expect_tok(Tok::Dedent, "the end of this interface's body (un-dent)")?;
        let end = methods
            .last()
            .map(|(n, f)| f.as_ref().map_or(n.span, |d| d.span))
            .unwrap_or(start);
        Ok(InterfaceDecl { name, methods, span: start.to(end) })
    }

    /// `class name ⏎ INDENT { has … | can <name> … block } DEDENT` (10.2).
    /// Methods are parsed straight into receiver-first functions (R-2: "methods
    /// are functions that take the object first"): the receiver param is named
    /// `myself` (10.2's reserved receiver word) and typed by the class's own
    /// name, so field reads on `myself` type-check like any struct's.
    fn class_decl(&mut self) -> PResult<ClassDecl> {
        let start = self.expect_kw(Kw::Class, "the word `class`")?;
        let name = self.parse_name("`class` must be followed by the class's name")?;
        // Header suffixes (10.5/10.6): `extends <class>` and
        // `does <interface>, <interface>` — comma or `and` separated.
        let mut extends = None;
        let mut does = Vec::new();
        loop {
            match self.peek_kw() {
                Some(Kw::Extends) => {
                    self.bump();
                    let base = self.parse_name("`extends` must be followed by the base class's name")?;
                    extends = Some(base);
                }
                Some(Kw::Does) => {
                    self.bump();
                    loop {
                        let iface =
                            self.parse_name("`does` must be followed by an interface's name")?;
                        does.push(iface);
                        if self.eat_tok(&Tok::Comma).is_none() && self.eat_kw(Kw::And).is_none() {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
        self.end_of_line("the class header")?;
        let Some(_is) = self.eat_tok(&Tok::Indent) else {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0202",
                "This class needs fields and methods indented under its header.",
                span,
            ));
            return Err(());
        };
        let mut fields = Vec::new();
        let mut constructions: Vec<FunctionDecl> = Vec::new();
        let mut methods: Vec<(Name, FunctionDecl)> = Vec::new();
        let mut deinit: Option<FunctionDecl> = None;
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            match self.peek_kw() {
                Some(Kw::Has) => {
                    let fstart = self.peek().1;
                    self.bump();
                    let fname = self.parse_name("`has` must be followed by the field's name")?;
                    self.expect_kw(Kw::OfType, "the words `of type`")?;
                    let ty = self.parse_type("After `of type`, write the field's type.")?;
                    let tspan = ty_span(&ty);
                    self.end_of_line("the field line")?;
                    fields.push(FieldDecl { name: fname, ty, span: fstart.to(tspan) });
                }
                Some(Kw::Construction) => {
                    // 10.3: `construction` — a named constructor. The clause
                    // body parses exactly like a function's (takes-clauses,
                    // then the indented block); the parser synthesizes a
                    // receiver-first function (R-2) named `construction <class>`
                    // whose implicit tail is `gives back myself` (10.3: the
                    // construction hands back the new object). Sema enforces
                    // the Swift rule on the body and dispatches the call by
                    // argument names (D-14).
                    let cstart = self.peek().1;
                    self.bump();
                    self.end_of_line("the `construction` header")?;
                    self.eat_newline_before_indent();
                    let block_open = self.eat_tok(&Tok::Indent);
                    let Some(block_open) = block_open else {
                        let (_, span) = self.peek();
                        self.errors.push(Diagnostic::error(
                            "E0202",
                            "This construction clause needs a body indented under its header.",
                            span,
                        ));
                        return Err(());
                    };
                    let mut params = Vec::new();
                    params.push(Param {
                        name: Name { words: vec!["myself".to_string()], span: cstart },
                        ty: TypeExpr::User(name.clone()),
                        span: cstart,
                    });
                    loop {
                        match self.peek_kw() {
                            Some(Kw::Takes) => {
                                self.bump();
                                params.push(self.takes_clause()?);
                            }
                            Some(Kw::CanFail) => {
                                self.bump();
                                self.end_of_line("the `can fail` clause")?;
                            }
                            _ => break,
                        }
                    }
                    let body = self.block_after_open(block_open)?;
                    // Implicit tail: the construction hands back the new
                    // object (10.3). The span is the body's so it lowers with
                    // the body.
                    let body_stmts = body.stmts.clone();
                    constructions.push(FunctionDecl {
                        name: Name {
                            words: vec![format!("construction {}", name.display())],
                            span: cstart,
                        },
                        params,
                        returns: Some(TypeExpr::User(name.clone())),
                        can_fail: false,
                        body: Block {
                            stmts: {
                                let mut s = body_stmts;
                                s.push(Stmt::GiveBack {
                                    value: Expr::Name {
                                        name: Name {
                                            words: vec!["myself".to_string()],
                                            span: cstart,
                                        },
                                        span: cstart,
                                    },
                                    span: cstart,
                                });
                                s
                            },
                            span: body.span,
                        },
                        span: cstart.to(self.peek().1),
                    });
                }
                Some(Kw::BeforeLastReferenceDisappears) => {
                    // 10.4: the finalizer clause. The parser synthesizes a
                    // receiver-first function (R-2) named `deinit <class>`;
                    // sema enforces R-20.3 (finalizers cannot fail) and the
                    // backends hook it to the last-reference drop. There is
                    // no `takes`/`returns`/`can fail` grammar on a finalizer
                    // — cleanup takes nothing, returns nothing, cannot fail.
                    let dstart = self.peek().1;
                    self.bump();
                    self.end_of_line("the `before last reference disappears` header")?;
                    self.eat_newline_before_indent();
                    let block_open = self.eat_tok(&Tok::Indent);
                    let Some(block_open) = block_open else {
                        let (_, span) = self.peek();
                        self.errors.push(Diagnostic::error(
                            "E0202",
                            "This finalizer needs a body indented under its header.",
                            span,
                        ));
                        return Err(());
                    };
                    let body = self.block_after_open(block_open)?;
                    let full_name = format!("deinit {}", name.display());
                    let fd = FunctionDecl {
                        name: Name { words: vec![full_name.clone()], span: dstart },
                        params: vec![Param {
                            name: Name { words: vec!["myself".to_string()], span: dstart },
                            ty: TypeExpr::User(name.clone()),
                            span: dstart,
                        }],
                        returns: None,
                        can_fail: false,
                        body,
                        span: dstart.to(self.peek().1),
                    };
                    if deinit.is_some() {
                        self.errors.push(Diagnostic::error(
                            "E0376",
                            format!("`{}` already has a `before last reference disappears` clause.", name.display()),
                            dstart,
                        ));
                        return Err(());
                    }
                    let _ = &full_name;
                    deinit = Some(fd);
                }
                Some(Kw::Can) => {
                    let mstart = self.peek().1;
                    self.bump();
                    let mut mname = self.parse_name("`can` must be followed by the method's name")?;
                    // §70's frozen form `can compare to`: the method name may
                    // end in the reserved word `to`. When the header line ends
                    // right after a dangling `to`, the word cannot open any
                    // clause — it joins the name (mirrors the call-site rule
                    // in `finish_call`).
                    // §70's frozen form `can compare to`: the method name may
                    // end in the reserved word `to`. The lexer's join rule
                    // (`joins_previous`) leaves such a header un-joined, so the
                    // dangling `to` sits at end-of-line — it cannot open any
                    // clause, so it joins the name.
                    if self.peek_kw() == Some(Kw::To) {
                        let to_span = self.bump().1;
                        mname.words.push("to".to_string());
                        mname.span = mname.span.to(to_span);
                    }
                    self.end_of_line("the method header")?;
                    // Method body with the function clause system (10.2: `can`
                    // methods may declare `can fail`) — same layout as 7.8.
                    self.eat_newline_before_indent();
                    let block_open = self.eat_tok(&Tok::Indent);
                    let Some(block_open) = block_open else {
                        let (_, span) = self.peek();
                        self.errors.push(Diagnostic::error(
                            "E0202",
                            "This method needs a body indented under its header.",
                            span,
                        ));
                        return Err(());
                    };
                    let mut params = Vec::new();
                    // R-2 lowering: the receiver leads the parameter list, so
                    // `myself` is in scope in the body exactly like any
                    // parameter (10.2's reserved receiver word).
                    params.push(Param {
                        name: Name { words: vec!["myself".to_string()], span: mname.span },
                        ty: TypeExpr::User(name.clone()),
                        span: mname.span,
                    });
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
                                let ty =
                                    self.parse_type("After `returns`, write a type (like `a number`).")?;
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
                    let body = self.block_after_open(block_open)?;
                    // R-2 lowering: the receiver leads the parameter list. The
                    // mangled name is unspellable as a user call head (`method`
                    // is reserved once a class exists) — sema dispatches method
                    // calls on the receiver's type.
                    let full_name = format!("method {} {}", name.display(), mname.display());
                    let fd = FunctionDecl {
                        name: Name { words: vec![full_name], span: mname.span },
                        params,
                        returns,
                        can_fail,
                        body,
                        span: mstart.to(self.peek().1),
                    };
                    methods.push((mname, fd));
                }
                _ => {
                    let (item, span) = self.peek();
                    self.errors.push(
                        Diagnostic::error("E0201", "I expected `has` or `can` here.", span)
                            .with_explanation(
                                "A class's body is `has <field> of type <T>` fields and `can <name>` methods.",
                            )
                            .with_note(format!("found: {}", item_text(item))),
                    );
                    return Err(());
                }
            }
        }
        self.expect_tok(Tok::Dedent, "the end of this class's body (un-dent)")?;
        let end = methods
            .last()
            .map(|(_, f)| f.span)
            .or_else(|| fields.last().map(|f| f.span))
            .unwrap_or(start);
        Ok(ClassDecl { name, fields, constructions, methods, extends, does, deinit, span: start.to(end) })
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

    /// `kind name ⏎ INDENT { is a variant [with f of type T …] ⏎ } DEDENT`
    /// (7.12) — the sum-type declaration. One variant per line; field lines
    /// mirror `structure`'s `of type` annotations.
    fn kind_decl(&mut self) -> PResult<KindDecl> {
        let start = self.expect_kw(Kw::Kind, "the word `kind`")?;
        let name = self.parse_name("`kind` must be followed by a name")?;
        self.end_of_line("the kind header")?;
        let Some(_is) = self.eat_tok(&Tok::Indent) else {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0202",
                "This kind needs its variants indented under the header.",
                span,
            ));
            return Err(());
        };
        let mut variants = Vec::new();
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            let vstart = self.peek().1;
            self.expect_kw(Kw::IsA, "the words `is a` (each kind variant starts with `is a`)")?;
            let vname = self.parse_name("`is a` must be followed by the variant's name")?;
            let mut fields = Vec::new();
            // `with radius of type number and height of type number` — the
            // same with/and spelling as construction (7.12's own example).
            while self.peek_kw() == Some(Kw::With) || self.peek_kw() == Some(Kw::And) {
                self.bump();
                let fname = self.parse_name("the variant field needs a name")?;
                self.expect_kw(Kw::OfType, "the words `of type`")?;
                let ty = self.parse_type("After `of type`, write the field's type.")?;
                let tspan = ty_span(&ty);
                fields.push((fname, ty, tspan));
            }
            self.end_of_line("the variant line")?;
            let end = fields.last().map(|(_, _, s)| *s).unwrap_or(vname.span);
            variants.push(VariantDecl { name: vname, fields, span: vstart.to(end) });
        }
        self.expect_tok(Tok::Dedent, "the end of this kind's variant list (un-dent)")?;
        let end = variants.last().map(|v| v.span).unwrap_or(start);
        Ok(KindDecl { name, variants, span: start.to(end) })
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
        // Layout (7.0.2): a block's last statement eats its own trailing
        // newline, so after the Dedent there may be no Newline left for the
        // enclosing statement's `end_of_line`. Leave the flag set so that
        // `end_of_line` accepts this position (11.1's block lambda:
        // `make f equal to a function taking n …` followed by more lines).
        self.closing_dedent = true;
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
        } else if self.closing_dedent {
            // Just closed an indented block inside an expression — the
            // newline was the body's last statement's own. The line ends here.
            self.closing_dedent = false;
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
        // Scope the just-closed-block flag to one statement: it is consumed
        // by this statement's own `end_of_line`, never leaks to the next.
        self.closing_dedent = false;
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
                self.end_of_line("`gives back`")?;
                Ok(Some(Stmt::GiveBack { value, span }))
            }
            Item::Kw(Kw::FailWith) => {
                self.bump();
                let value = self.expr()?;
                self.end_of_line("`fail with`")?;
                Ok(Some(Stmt::FailWith { value, span }))
            }
            Item::Kw(Kw::Attempt) => self.attempt_stmt().map(Some),
            Item::Kw(Kw::StartTask) => self.start_task_stmt().map(Some),
            Item::Kw(Kw::WaitForAllTasks) => {
                self.bump();
                self.end_of_line("`wait for all tasks`")?;
                Ok(Some(Stmt::WaitForAllTasks { span }))
            }
            Item::Kw(Kw::Match) => self.match_stmt().map(Some),
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

    /// `start a task [keep going]` (14.2): one spawn per statement. The
    /// `keep going` suffix rides the spawn's header line, before the body
    /// block (the demo's frozen surface). The body is the task's region.
    fn start_task_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::StartTask, "the words `start a task`")?;
        let keep_going = if self.peek_kw() == Some(Kw::KeepGoing) {
            self.bump();
            true
        } else {
            false
        };
        let body = self.block("After `start a task`, indent the task's body.")?;
        let span = start.to(body.span);
        Ok(Stmt::StartTask { body, keep_going, span })
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

    /// `match expr ⏎ INDENT { when pattern ⏎ block } [otherwise ⏎ block] DEDENT`
    /// (7.12/7.15). Patterns and their bodies are block-structured: each
    /// `when` line is followed by its indented block.
    fn match_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::Match, "the word `match`")?;
        let scrutinee = self.expr()?;
        self.end_of_line("the `match` header")?;
        let Some(_is) = self.eat_tok(&Tok::Indent) else {
            let (_, span) = self.peek();
            self.errors.push(Diagnostic::error(
                "E0202",
                "This `match` needs its `when` arms indented under the header.",
                span,
            ));
            return Err(());
        };
        let mut arms = Vec::new();
        let mut otherwise = None;
        loop {
            if self.at_dedent() || self.at_eof() {
                break;
            }
            if self.at_newline() {
                self.bump();
                continue;
            }
            match self.peek_kw() {
                Some(Kw::When) => {
                    self.bump();
                    let pattern = self.parse_pattern()?;
                    self.end_of_line("the `when` line")?;
                    let body = self.block("The `when` arm needs a body indented under it.")?;
                    arms.push((pattern, body));
                }
                Some(Kw::Otherwise) => {
                    self.bump();
                    otherwise = Some(self.block("The `otherwise` arm needs a body indented under it.")?);
                }
                _ => {
                    let (item, span) = self.peek();
                    self.errors.push(
                        Diagnostic::error("E0201", "Inside a `match`, each arm starts with `when`.", span)
                            .with_note(format!("found: {}", item_text(item))),
                    );
                    return Err(());
                }
            }
        }
        self.expect_tok(Tok::Dedent, "the end of this match (un-dent)")?;
        let end = otherwise
            .as_ref()
            .map(|b| b.span)
            .or_else(|| arms.last().map(|(_, b)| b.span))
            .unwrap_or(start);
        Ok(Stmt::Match { scrutinee, arms, otherwise, span: start.to(end) })
    }

    /// One `match` pattern (7.15's `pattern` production):
    /// literal | name | `a <usertype> [with <field> <pattern> and …]`
    /// | `nothing` | `something with value <pattern>` | `a pair of <pattern> and <pattern>`.
    fn parse_pattern(&mut self) -> PResult<Pattern> {
        let (item, span) = self.peek();
        match item {
            // `when nothing` (8.5).
            Item::Kw(Kw::Nothing) => {
                self.bump();
                Ok(Pattern::Literal { value: PatternLiteral::Nothing, span })
            }
            // `when something` / `when something with value <pattern>` (8.5).
            // The phrase-token `something with value` lexes as ONE item, so it
            // is matched before the bare `something` word.
            Item::Kw(Kw::SomethingWithValue) => {
                self.bump();
                let inner = self.parse_pattern()?;
                let end = pattern_span(&inner);
                Ok(Pattern::Something { inner: Box::new(inner), span: span.to(end) })
            }
            Item::Kw(Kw::Something) => {
                self.bump();
                let name = Name { words: vec!["something".to_string()], span };
                Ok(Pattern::Name { name })
            }
            // Literals.
            Item::Tok(Tok::Int(v)) => {
                let v = *v;
                self.bump();
                Ok(Pattern::Literal { value: PatternLiteral::Int(v), span })
            }
            Item::Tok(Tok::Float(v)) => {
                let v = *v;
                self.bump();
                Ok(Pattern::Literal { value: PatternLiteral::Float(v), span })
            }
            Item::Tok(Tok::Text(s)) => {
                let s = s.clone();
                self.bump();
                Ok(Pattern::Literal { value: PatternLiteral::Text(s), span })
            }
            Item::Kw(Kw::True) => {
                self.bump();
                Ok(Pattern::Literal { value: PatternLiteral::Bool(true), span })
            }
            Item::Kw(Kw::False) => {
                self.bump();
                Ok(Pattern::Literal { value: PatternLiteral::Bool(false), span })
            }
            // `a pair of <pattern> and <pattern>` — pair destructuring.
            Item::Kw(Kw::APairOf) => {
                self.bump();
                let first = self.parse_pattern()?;
                self.expect_kw(Kw::And, "the word `and` (joining the pair's patterns)")?;
                let second = self.parse_pattern()?;
                let end = pattern_span(&second);
                Ok(Pattern::Pair { first: Box::new(first), second: Box::new(second), span: span.to(end) })
            }
            // Article: a variant destructure (`a circle with radius r`) or the
            // article-as-name fallback (7.9's contextual articles).
            Item::Kw(Kw::A) | Item::Kw(Kw::An) => {
                if matches!(self.items.get(self.pos + 1), Some((Item::Tok(Tok::WordRun(_)), _))) {
                    self.bump();
                    let name = self.parse_name("`a` must be followed by the variant's name")?;
                    let mut fields = Vec::new();
                    while self.peek_kw() == Some(Kw::With) || self.peek_kw() == Some(Kw::And) {
                        self.bump();
                        let fname = self.parse_name("the pattern field needs a name")?;
                        // §7.12's pattern grammar: `with` takes *name pairs* —
                        // the field is the leading words, the binding the last
                        // word when they differ (`with radius r` = field
                        // `radius`, binding `r`). A single word means the
                        // binding *is* the field (`with radius`). The §7.12
                        // examples spell it both ways.
                        let (fname, sub): (Name, Pattern) = if fname.words.len() >= 2 {
                            let last = fname.words.last().unwrap().clone();
                            let span = fname.span;
                            (
                                Name { words: fname.words[..fname.words.len() - 1].to_vec(), span },
                                Pattern::Name { name: Name { words: vec![last], span } },
                            )
                        } else {
                            let f = fname.clone();
                            (f.clone(), Pattern::Name { name: f })
                        };
                        fields.push((fname, sub));
                    }
                    let end = fields
                        .last()
                        .map(|(_, p)| pattern_span(p))
                        .unwrap_or(name.span);
                    Ok(Pattern::Variant { name, fields, span: span.to(end) })
                } else {
                    let name = self.parse_name("a pattern")?;
                    Ok(Pattern::Name { name })
                }
            }
            // A bare name: binding, variant literal, or a multi-word variant
            // name whose fields follow (`when a rectangle with …` is handled
            // above; `when rectangle` stays a binding-shaped variant match).
            Item::Tok(Tok::WordRun(_)) => {
                let name = self.parse_name("a pattern")?;
                Ok(Pattern::Name { name })
            }
            _ => {
                self.errors.push(
                    Diagnostic::error("E0205", "I expected a pattern here.", span)
                        .with_explanation("A `when` pattern is a value to compare (`0`, `\"quit\"`), a name to bind, `nothing`, `something with value …`, or `a <variant> with …`.")
                        .with_note(format!("found: {}", item_text(item))),
                );
                Err(())
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
        self.continue_comparison(left)
    }

    /// The comparison operators that may follow an additive expression
    /// (`left` already parsed by the caller). Shared by `comparison` and
    /// by `finish_call`'s first argument.
    fn continue_comparison(&mut self, left: Expr) -> PResult<Expr> {
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
            // 14.3: `a channel of T` in value position — the type phrase IS
            // the construction; the element type must still parse.
            Item::Kw(Kw::AChannelOf) => {
                let start = self.expect_kw(Kw::AChannelOf, "the words `a channel of`")?;
                let elem = self.parse_type("After `a channel of`, write the message type.")?;
                let span = start.to(ty_span(&elem));
                Ok(Expr::ChannelLit { elem: Box::new(elem), span })
            }
            // The block lambda (11.1): `a function [taking n and m …] block`.
            Item::Kw(Kw::A) | Item::Kw(Kw::An)
                if matches!(self.items.get(self.pos + 1), Some((Item::Kw(Kw::Function), _))) =>
            {
                self.block_lambda()
            }
            // The inline lambda (11.1): `taking n giving back <comparison>` —
            // opened by `taking` in expression position.
            Item::Kw(Kw::Taking) => self.inline_lambda(),
            // The option construction (8.5/G-21): `something with value v` —
            // §8.5's value sentence made expression-shaped. The phrase lexes
            // as ONE token; the value binds at the additive level (R-4: `and`
            // stays an argument separator).
            Item::Kw(Kw::SomethingWithValue) => {
                let start = self.bump().1;
                let value = self.additive()?;
                let end = expr_span(&value);
                Ok(Expr::SomeValue { value: Box::new(value), span: start.to(end) })
            }
            // `it` is a reserved word (11.1) — the implicit combinator
            // parameter, in scope only where a lambda binds it (sema checks).
            Item::Kw(Kw::It) => {
                self.bump();
                Ok(Expr::Name { name: Name { words: vec!["it".to_string()], span }, span })
            }
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
            // Class construction (10.2/10.3): `a new counter with count 5` —
            // the same reader shape as struct construction; sema checks full
            // field initialization (the Swift rule) against the class table.
            Item::Kw(Kw::ANew) => self.new_object(),
            Item::Tok(Tok::WordRun(_)) => self.try_name_or_call(),
            // Conversion heads are type words used as call heads: `number from answer` (D-39).
            Item::Kw(Kw::Number)
            | Item::Kw(Kw::Decimal)
            | Item::Kw(Kw::Text)
            | Item::Kw(Kw::Boolean)
            | Item::Kw(Kw::Random) => self.try_call_from_head(),
            // Construction (7.11 structlit / 7.12 variantlit): `a player with
            // name "bo" and score 0` — the same reader shape; sema resolves
            // which table (structure or kind variant) owns the name. An
            // article followed by a name opens a construction; otherwise the
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
                // The body is re-parsed as its own source fragment, so its
                // spans start at 0 — pad the fragment with the newlines that
                // precede the string in the real file and every diagnostic
                // inside `{…}` lands on the true line and column (M2's
                // "exactly where" contract; multi-line strings included).
                let pad = "\n".repeat(span.start.min(self.src.len()));
                let padded = format!("{pad}{inner}");
                let (program, mut errs) = parse(&padded);
                let shift = pad.len();
                for e in &mut errs {
                    e.span.start += shift;
                    e.span.end += shift;
                    for (ls, _) in &mut e.labels {
                        ls.start += shift;
                        ls.end += shift;
                    }
                }
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
    /// Does a flowcall ARGUMENT open after a name run? `and` does NOT start
    /// one: the andexpr ladder owns boolean `and` at parse level, and sema's
    /// G-22 arm splits `split line and ","` into call arguments when callee
    /// resolution calls for it (the parser cannot know bound names).
    fn starts_flowcall_argument(&self) -> bool {
        self.starts_argument()
    }

    fn try_name_or_call(&mut self) -> PResult<Expr> {
        let callee = self.parse_name("a value or a call")?;
        // 7.1/S-14: `say`/`ask` are ordinary standard-module functions whose
        // readable sentence form puts the value after the head word. When a
        // multi-word run starts with one of them and an infix operator
        // follows (`say age plus 1`), the operator belongs to the *argument*
        // — split the head off and let the additive ladder read the rest
        // (say(age + 1)); `(say age) plus 1` is not a derivable reading
        // because `say` returns nothing. A comparison, though, is exactly
        // R-4's ambiguous spelling: diagnose, never misparse.
        if callee.words.len() >= 2
            && matches!(callee.words[0].as_str(), "say" | "ask")
            && matches!(
                self.peek_kw(),
                Some(
                    Kw::Plus
                        | Kw::Minus
                        | Kw::TimesWord
                        | Kw::DividedBy
                        | Kw::DividedEvenlyBy
                        | Kw::Modulo
                )
            )
        {
            let tail_span = callee.span;
            self.push_back_name(Name { words: callee.words[1..].to_vec(), span: tail_span });
            let head = Name { words: vec![callee.words[0].clone()], span: callee.span };
            return self.finish_call(head);
        }
        if callee.words.len() >= 2
            && matches!(callee.words[0].as_str(), "say" | "ask")
            && matches!(
                self.peek_kw(),
                Some(
                    Kw::CmpEqualTo
                        | Kw::CmpNotEqualTo
                        | Kw::CmpGreaterThan
                        | Kw::CmpLessThan
                        | Kw::CmpAtLeast
                        | Kw::CmpAtMost
                        | Kw::IsNothing
                        | Kw::IsSomething
                )
            )
        {
            let (_, cspan) = self.peek();
            self.errors.push(
                Diagnostic::error(
                    "E0210",
                    "A comparison cannot end a `say`/`ask` sentence — it is ambiguous here.",
                    cspan,
                )
                .with_note("arguments bind at the additive level (R-4), so `is equal to`, `is less than`, and their siblings would be read as a comparison on the whole call")
                .with_fix("wrap the comparison in parentheses so it names its own value: say (count is less than 3)"),
            );
            return Err(());
        }
        // no argument tokens follow → plain name
        if !self.starts_flowcall_argument() {
            let span = callee.span;
            return Ok(Expr::Name { name: callee, span });
        }
        // R-3: `using`/`where` are the SENTENCE's call suffixes. A multi-word
        // run (`map xs using f` → run `map xs`) carries them — but a
        // single-word value followed by `using` (`combine xs with start seen
        // using f`: the run is just `seen`) is the with-labeled argument, and
        // the suffix belongs to the enclosing sentence call, not to `seen`.
        let peek_using = self.peek_kw() == Some(Kw::Using) || self.peek_kw() == Some(Kw::Where);
        if callee.words.len() == 1 && peek_using && !self.starts_additive() {
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

    /// The block lambda (11.1): `a function [taking n and m] ⏎ INDENT …body…
    /// DEDENT` — the parameters come from the optional `taking` clause on the
    /// header line; the body is the indented block (7.15's funclit: "the
    /// block may open with takes/returns/can clauses (11.1: `twice`)" — those
    /// typed-clause forms are an M2 signature feature; M1 infers parameters).
    fn block_lambda(&mut self) -> PResult<Expr> {
        let start = self.peek().1;
        self.eat_kw(Kw::A);
        self.eat_kw(Kw::An);
        self.expect_kw(Kw::Function, "the words `a function`")?;
        let mut params: Vec<Name> = Vec::new();
        if self.eat_kw(Kw::Taking).is_some() {
            params.push(self.parse_name("`taking` must be followed by the parameter's name")?);
            while self.eat_kw(Kw::And).is_some() {
                params.push(self.parse_name("`and` must be followed by the next parameter's name")?);
            }
        }
        let body = self.block("The `a function` lambda needs its body indented under it.")?;
        Ok(Expr::Lambda { params, body: LambdaBody::Block(body.clone()), span: start.to(body.span) })
    }

    /// The inline lambda (11.1/R-3): `taking <name> [and <name> …] giving
    /// back <comparison>` — the body is one comparison-level expression
    /// (bounded; boolean bodies use the block form).
    fn inline_lambda(&mut self) -> PResult<Expr> {
        let start = self.expect_kw(Kw::Taking, "the word `taking`")?;
        let mut params = vec![self.parse_name("`taking` must be followed by the parameter's name")?];
        while self.eat_kw(Kw::And).is_some() {
            params.push(self.parse_name("`and` must be followed by the next parameter's name")?);
        }
        self.expect_kw(Kw::GivingBack, "the words `giving back`")?;
        let body = self.comparison()?;
        let end = expr_span(&body);
        Ok(Expr::Lambda {
            params,
            body: LambdaBody::Inline(Box::new(body)),
            span: start.to(end),
        })
    }

    /// The `using` argument (7.15's `lambda` production): a bare comparison
    /// (`using double` passes the function itself; `using it plus 5` is the
    /// implicit-`it` lambda — `it` and the preceding `with`-bound names are
    /// in scope) or the inline `taking … giving back …` form.
    fn parse_lambda(&mut self) -> PResult<Expr> {
        if self.peek_kw() == Some(Kw::Taking) {
            self.inline_lambda()
        } else {
            self.comparison()
        }
    }

    /// Construction (7.11 structlit): `a player with name "bo" and score 0` —
    /// the second+ `with` spells `and` (7.11). Requires a user-type name.
    fn struct_lit(&mut self) -> PResult<Expr> {
        let start = self.peek().1;
        self.eat_kw(Kw::A);
        self.eat_kw(Kw::An);
        let name = self.parse_name("`a` must be followed by the structure's name")?;
        let fields = self.construction_fields(&name)?;
        let end = fields
            .last()
            .map(|(_, v)| expr_span(v))
            .unwrap_or(start);
        Ok(Expr::StructLit { name, fields, span: start.to(end) })
    }

    /// `a new <class> with <field> <value> [and <field> <value>]…` (10.2):
    /// the class-construction reader. Field spelling, and-conjunction, and
    /// value-head splitting match `struct_lit`'s construction reader exactly
    /// — the two differ only in the keyword that opens them (`a new` vs
    /// `a`) and the node they build.
    fn new_object(&mut self) -> PResult<Expr> {
        let start = self.bump().1;
        let name = self.parse_name("`a new` must be followed by the class's name")?;
        let fields = self.construction_fields(&name)?;
        let end = fields
            .last()
            .map(|(_, v)| expr_span(v))
            .unwrap_or(start);
        Ok(Expr::NewObject { name, fields, span: start.to(end) })
    }

    /// The `with <field> <value> [and <field> <value>]…` reader shared by
    /// struct (7.11) and class (10.2) construction. Caller has consumed the
    /// article (and `new`) and read the type's name.
    fn construction_fields(&mut self, _name: &Name) -> PResult<Vec<(Name, Expr)>> {
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
            // The field value's head word can be lexed INTO the field-name
            // run (`a ok with label fields at 0` — the lexer merges `label
            // fields` because both words are plain). Names and values never
            // contain a flowcall preposition (`of/at/from/to` is a closed
            // reserved set, 7.15), so when the run is followed by a prep the
            // run's last word is provably the value's head: split it off and
            // re-parse the value from there. The same holds when the run is
            // followed by the keyword `and` (the next field's conjunction —
            // `a good with filepath fp and folder "misc"`) or by the end of
            // the line (`a broken with reason row`): a field value can never
            // be empty, and a value can never START with `and` or end-of-line,
            // so the last run word is the earliest provable value head.
            // Deterministic, no backtracking (the value still must parse
            // from that word).
            let at_boundary = matches!(self.peek_kw(), Some(Kw::Of | Kw::At | Kw::From | Kw::To | Kw::And))
                || matches!(self.peek().0, Item::Tok(Tok::Newline | Tok::Dedent | Tok::Eof));
            let fname = if fname.words.len() > 1 && at_boundary {
                let mut words = fname.words.clone();
                let head = words.pop().unwrap();
                let head_span_end = fname.span.end;
                let word_len = head.len();
                let head_span = Span { start: head_span_end - word_len, end: head_span_end };
                // `fname` keeps the words BEFORE the value head (`label` in
                // `label fields at 0`); the head word goes back on the stream
                // so the value parses from it.
                let rest = Name { words, span: fname.span };
                let head_name = Name { words: vec![head], span: head_span };
                self.push_back_name(head_name);
                rest
            } else {
                fname
            };
            if !self.starts_additive() {
                let (_, span) = self.peek();
                self.errors.push(Diagnostic::error(
                    "E0207",
                    format!("`{} {}` needs a value.", _name.display(), fname.display()),
                    span,
                ));
                return Err(());
            }
            let value = self.additive()?;
            // The field value's own flowcall greedily consumed any `and`
            // continuation (`a ok with name p at 0 and score q at 1` — the
            // inner `p at 0` call holds `and score q at 1`). Inside a
            // construction, an and-arg shaped like the NEXT FIELD (`and
            // score 0`, 7.11's own exemplar) belongs to the construction,
            // not the call: pop field-shaped leading and-args out into
            // fields. Non-field-shaped and-args (`and ","` — `split line
            // and ","` as a value) stay the call's arguments.
            let (value, promoted) = split_field_continuation(value);
            fields.push((fname, value));
            fields.extend(promoted);
            // After a field value, an `and` continues the construction ONLY
            // when it opens the next `name <additive>` field (7.11's exemplar:
            // `a player with name "bo" and score 0`). An `and` followed by an
            // additive WITHOUT a name head belongs to the field value's own
            // flowcall continuation (`… with label fields at 0 and num 2` is
            // label = fields(0, num(2)) when `num` names nothing bound here —
            // wait: that reading is ambiguous, so the tie-break is positional:
            // `and <name> <additive>` where the name is a plain single-word
            // run followed by an additive value is the NEXT FIELD; anything
            // else (`and "x"`, `and (a plus b)`) is the call's argument list.
            // This keeps the construction reading for exactly the shape the
            // grammar's exemplar shows, and never re-binds an `and`-argument.
            if self.peek_kw() == Some(Kw::And) {
                let next_is_field = self.peek_and_then_field_name();
                if !next_is_field {
                    break;
                }
            }
        }
        Ok(fields)
    }

    /// After `and` (already peeked): does a `name <additive>` field follow?
    /// The lookahead is two tokens: a single-word run (the field name) then
    /// something additive-level. Multi-word runs after `and` in a construction
    /// are call continuations (`a ok with label p and size of q` — `size of
    /// q` continues the `label` field's value only when `size` is a bound
    /// name, which the parser cannot know; the single-word field-name shape
    /// is the grammar's own exemplar form). A known PREP keyword opens the
    /// flowing-read value form (`and size of q` = the field `size` reading
    /// `q`), so preps also count as value openers.
    fn peek_and_then_field_name(&self) -> bool {
        // self.items[self.pos] is the `and` keyword's item slot? peek_kw() saw
        // `and` AT pos; the next item is pos+1.
        let name_item = self.items.get(self.pos + 1).map(|(i, _)| i);
        let value_item = self.items.get(self.pos + 2).map(|(i, _)| i);
        let single_word_name = matches!(
            name_item,
            Some(Item::Tok(Tok::WordRun(w))) if w.len() == 1
        );
        if !single_word_name {
            return false;
        }
        match value_item {
            // Ints, floats, texts, parens open values; articles open nested
            // constructions (`and width a number` is a value, not a field —
            // but `a list of …` also opens a value; both are additive).
            Some(Item::Tok(Tok::Int(_)))
            | Some(Item::Tok(Tok::Float(_)))
            | Some(Item::Tok(Tok::Text(_)))
            | Some(Item::Tok(Tok::LParen))
            | Some(Item::Tok(Tok::Minus)) => true,
            Some(Item::Kw(
                Kw::True | Kw::False | Kw::Nothing | Kw::AListOf | Kw::AMapFrom | Kw::APairOf,
            )) => true,
            Some(Item::Kw(Kw::Of | Kw::At | Kw::From | Kw::To)) => true,
            _ => false,
        }
    }
}

/// Split a construction field value's trailing field-shaped and-args into
/// (value, next_fields). See `struct_lit`'s call site for the rule.
fn split_field_continuation(value: Expr) -> (Expr, Vec<(Name, Expr)>) {
    let Expr::Call(mut c) = value else { return (value, Vec::new()) };
    if c.using_arg.is_some() || c.where_expr.is_some() || !c.with_args.is_empty() {
        return (Expr::Call(c), Vec::new());
    }
    let mut promoted: Vec<(Name, Expr)> = Vec::new();
    while let Some(arg) = c.and_args.first() {
        match field_shaped(&arg.expr) {
            Some((fname, fvalue)) => {
                c.and_args.remove(0);
                promoted.push((fname, fvalue));
            }
            None => break,
        }
    }
    (Expr::Call(c), promoted)
}

/// Is this and-arg shaped like the construction's next field? Two shapes
/// (both the grammar exemplar's own: `a player with name "bo" and score 0`):
/// `field value` (single-word callee + first positional: `num 2`) and
/// `field valuehead rest…` (multi-word callee + prep: `score fields at 1`,
/// the same value-head-rides-the-name-run rule as the field-name split).
fn field_shaped(e: &Expr) -> Option<(Name, Expr)> {
    let Expr::Call(c) = e else { return None };
    if c.using_arg.is_some() || c.where_expr.is_some() || !c.with_args.is_empty() || !c.and_args.is_empty() {
        return None;
    }
    if c.preps.is_empty() {
        // `field value`: one-word callee, one positional value.
        if c.callee.words.len() == 1 {
            if let Some(first) = &c.first {
                return Some((c.callee.clone(), first.expr.as_ref().clone()));
            }
        }
        return None;
    }
    // `field valuehead rest…`: the field name rides the callee run's head;
    // the remaining words + preps are the value (a prep-continuing call).
    if c.first.is_none() && c.callee.words.len() >= 2 {
        let mut words = c.callee.words.clone();
        let fname = words.remove(0);
        let fvalue = Expr::Call(Box::new(ast::CallExpr {
            callee: Name { words, span: c.callee.span },
            first: None,
            preps: c.preps.clone(),
            and_args: Vec::new(),
            with_args: Vec::new(),
            using_arg: None,
            where_expr: None,
            span: c.span,
        }));
        return Some((Name { words: vec![fname], span: c.callee.span }, fvalue));
    }
    None
}

impl Parser<'_> {
    /// additive-level values (R-4), prepositional (`of/at/from/to` + value —
    /// `name of p`, `random from 1 to 6`), or the `using`/`where` call
    /// suffixes (R-3 — `map scores using double`, `keep scores where …`).
    fn starts_argument(&self) -> bool {
        self.starts_additive()
            || matches!(
                self.peek_kw(),
                Some(Kw::Of | Kw::At | Kw::From | Kw::To | Kw::Using | Kw::Where | Kw::With)
            )
    }

    /// The unified call grammar (7.15, R-1): first positional arg, prepositional
    /// args, `and`-separated args, then `with`-labeled suffixes.
    fn finish_call(&mut self, mut callee: Name) -> PResult<Expr> {
        let start = callee.span;
        let first = if self.starts_additive() {
            // R-4: arguments bind at the additive level — a comparison that
            // follows belongs to the enclosing context (7.15:
            // `check that double 2 is equal to 4` compares the whole call;
            // `say (count is less than 3)` parenthesizes).
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
            // §70's frozen method-call form `compare to of v with other "y"`:
            // the method's name absorbs the prep word (`compare to`). With no
            // first argument the prep would dangle — a call with no receiver —
            // so the bare-prep shape cannot be a real argument position: the
            // word joins the callee's name instead, and the NEXT prep is the
            // argument's. (`set <name> to …` never reaches `finish_call`; a
            // first positional argument or another prep always precedes any
            // real `to` argument.)
            if prep == Prep::To && first.is_none() && preps.is_empty() {
                let to_span = self.items[self.pos - 1].1;
                callee.words.push("to".to_string());
                callee.span = callee.span.to(to_span);
                continue;
            }
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
        // 7.9/7.11: a labeled-argument chain continues with `and` when the
        // `and` opens the next `name <additive>` label — the construction
        // exemplar's own shape (`greet with name "bo" and punctuation "!"`).
        // Any other `and` (value continuation, bare argument) was already
        // consumed by the and-loop above or ends the call.
        loop {
            let is_with = self.peek_kw() == Some(Kw::With);
            let is_and_next_label = self.peek_kw() == Some(Kw::And)
                && !with_args.is_empty()
                && self.peek_and_then_field_name();
            if !is_with && !is_and_next_label {
                break;
            }
            self.bump();
            let label = self.parse_name("`with` must be followed by the argument's name")?;
            // The label's value-head word can be lexed INTO the label run:
            // `with start seen using …` reads the run as one name (`start
            // seen`). A call argument can never be empty, so when the run is
            // followed by a keyword boundary (`using`/`where`/`and`/prep or
            // end-of-line), the run's last word is provably the value's head
            // — split it off (the same deterministic rule the construction
            // fields use above).
            let label = if label.words.len() > 1 {
                let after_value = matches!(
                    self.peek_kw(),
                    Some(Kw::Using | Kw::Where | Kw::And | Kw::Of | Kw::At | Kw::From | Kw::To)
                ) || self.at_newline() || self.at_dedent() || self.at_eof();
                if after_value && !self.starts_additive() {
                    let mut words = label.words.clone();
                    let head = words.pop().unwrap();
                    let head_len = head.len();
                    let head_span = Span { start: label.span.end - head_len, end: label.span.end };
                    let value_name = Name { words: vec![head], span: head_span };
                    self.push_back_name(value_name);
                    Name { words, span: Span { start: label.span.start, end: head_span.start } }
                } else {
                    label
                }
            } else {
                label
            };
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
        // `using <lambda>` (R-3/11.2): the function-valued argument, one per
        // call, after every positional/prepositional/labeled argument.
        let mut using_arg = None;
        if self.peek_kw() == Some(Kw::Using) {
            self.bump();
            using_arg = Some(Box::new(self.parse_lambda()?));
        }
        // `where <orexpr>` (R-3): the filter sugar — `keep scores where it is
        // at least 80` — one desugaring rule in sema (`using taking it giving
        // back <expr>`).
        let mut where_expr = None;
        if self.peek_kw() == Some(Kw::Where) {
            self.bump();
            where_expr = Some(Box::new(self.orexpr()?));
        }
        let end = where_expr
            .as_deref()
            .map(expr_span)
            .or_else(|| using_arg.as_deref().map(expr_span))
            .or_else(|| and_args.last().map(|a| a.span))
            .or_else(|| with_args.last().map(|(_, a)| a.span))
            .or_else(|| preps.last().map(|(_, a)| a.span))
            .or(first.as_deref().map(|a| a.span))
            .unwrap_or(start);
        self.check_ambiguous_or(&callee)?;
        Ok(Expr::Call(Box::new(CallExpr { callee, first, preps, and_args, with_args, using_arg, where_expr, span: start.to(end) })))
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
                | Kw::Random
                | Kw::It
                | Kw::Taking,
            ) => true,
            // The block lambda opens an argument: `twice with f a function …`.
            Item::Kw(Kw::A) | Item::Kw(Kw::An)
                if matches!(self.items.get(self.pos + 1), Some((Item::Kw(Kw::Function), _))) => true,
            Item::Tok(Tok::WordRun(_)) => true,
            _ => false,
        }
    }

    /// Re-insert a Name's words into the token stream (the construction
    /// field-value split: after carving the value's head word off a merged
    /// run, the remaining words go back so the value re-parses from them).
    /// Insertion order preserves the stream's left-to-right read.
    fn push_back_name(&mut self, name: Name) {
        let mut items: Vec<(Item, Span)> = Vec::with_capacity(name.words.len());
        for w in &name.words {
            items.push((Item::Tok(Tok::WordRun(vec![w.clone()])), name.span));
        }
        for (item, span) in items.into_iter().rev() {
            self.items.insert(self.pos, (item, span));
        }
    }

    /// Parse a Name: a WordRun (one or more words), or the standalone article
    /// `a`/`an` used as a one-word name — §7.9's own example `bigger of a and b`
    /// uses `a` as a variable, so articles are *contextual*: they are articles
    /// in type/construction positions and names everywhere else.
    fn parse_name(&mut self, expecting: &str) -> PResult<Name> {        match self.peek() {
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
            _ => {                let (item, span) = self.peek();
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
        TypeExpr::List(t) | TypeExpr::Map(t, _) | TypeExpr::Pair(t, _) | TypeExpr::Channel(t) => {
            ty_span_b(t)
        }
        _ => Span::default(),
    }
}

fn ty_span_b(t: &TypeExpr) -> Span {
    ty_span(t)
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::NewObject { span, .. }
        | Expr::Int { span, .. }
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
        | Expr::ChannelLit { span, .. }
        | Expr::StructLit { span, .. }
        | Expr::VariantLit { span, .. }
        | Expr::SomeValue { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::AttemptExpr { span, .. } => *span,
        Expr::Call(c) => c.span,
    }
}

fn pattern_span(p: &Pattern) -> Span {
    match p {
        Pattern::Literal { span, .. }
        | Pattern::Variant { span, .. }
        | Pattern::Something { span, .. }
        | Pattern::Pair { span, .. } => *span,
        Pattern::Name { name } => name.span,
        Pattern::Wildcard => Span::default(),
    }
}

fn kw_text(kw: Kw) -> &'static str {
    use Kw::*;
    match kw {
        Extends => "extends",
        Interface => "interface",
        Does => "does",
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
        GiveBack => "gives back",
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
        Class => "class",
        Can => "can",
        Construction => "construction",
        BeforeLastReferenceDisappears => "before last reference disappears",
        ANew => "a new",
        Has => "has",
        OfType => "of type",
        WaitForAllTasks => "wait for all tasks",
        StartTask => "start a task",
        KeepGoing => "keep going",
        AChannelOf => "a channel of",
        Kind => "kind",
        Match => "match",
        When => "when",
        Taking => "taking",
        GivingBack => "giving back",
        Where => "where",
        It => "it",
        Something => "something",
        SomethingWithValue => "something with value",
        IsA => "is a",
        OrNothing => "or nothing",
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

    // ----- M1: kinds, match, patterns, option types, lambdas (7.12, 11.1, 8.5) -----

    #[test]
    fn kind_decl_with_variants() {
        let p = parse_ok(
            "kind shape\n    is a blank\n    is a circle with radius of type number\n",
        );
        match &p.items[0] {
            AstItem::Kind(k) => {
                assert_eq!(k.name.display(), "shape");
                assert_eq!(k.variants.len(), 2);
                assert_eq!(k.variants[0].name.display(), "blank");
                assert!(k.variants[0].fields.is_empty());
                assert_eq!(k.variants[1].name.display(), "circle");
                assert_eq!(k.variants[1].fields.len(), 1);
                assert_eq!(k.variants[1].fields[0].0.display(), "radius");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn match_with_when_arms() {
        let p = parse_ok(
            "match s\n    when blank\n        say \"empty\"\n    when something with value n\n        say n\n",
        );
        match first_stmt(&p) {
            Stmt::Match { scrutinee, arms, .. } => {
                assert!(matches!(scrutinee, Expr::Name { .. }));
                assert_eq!(arms.len(), 2);
                assert!(matches!(arms[0].0, Pattern::Name { .. }));
                assert!(matches!(arms[1].0, Pattern::Something { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn match_with_literals_and_otherwise() {
        let p = parse_ok(
            "match x\n    when 0\n        say \"zero\"\n    when \"quit\"\n        say \"bye\"\n    otherwise\n        say \"other\"\n",
        );
        match first_stmt(&p) {
            Stmt::Match { arms, otherwise, .. } => {
                assert_eq!(arms.len(), 2);
                assert!(matches!(&arms[0].0, Pattern::Literal { value: PatternLiteral::Int(0), .. }));
                assert!(matches!(&arms[1].0, Pattern::Literal { value: PatternLiteral::Text(_), .. }));
                assert!(otherwise.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn match_with_variant_and_pair_patterns() {
        let p = parse_ok(
            "match s\n    when a circle with radius r\n        say r\n    when a pair of a and b\n        say first of pt\n",
        );
        match first_stmt(&p) {
            Stmt::Match { arms, .. } => {
                assert!(matches!(&arms[0].0, Pattern::Variant { name, fields, .. }
                    if name.display() == "circle" && fields.len() == 1));
                assert!(matches!(&arms[1].0, Pattern::Pair { .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn option_type_spellings_parse() {
        // Compact: `T?`. Word: `a T or nothing`. One type, two spellings (R-18).
        let p = parse_ok("function maybe\n    returns text?\n    gives back nothing");
        match &p.items[0] {
            AstItem::Function(f) => match &f.returns {
                Some(TypeExpr::OptionT(inner)) => assert!(matches!(**inner, TypeExpr::Text)),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
        let p = parse_ok("function maybe\n    returns a text or nothing\n    gives back nothing");
        match &p.items[0] {
            AstItem::Function(f) => {
                assert!(matches!(f.returns, Some(TypeExpr::OptionT(_))));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn variant_construction_parses_like_structlit() {
        // The reader shape is uniform (7.11/7.12): `a <name> with …`. The
        // parser cannot know whether `circle` is a structure or a kind
        // variant — sema reclassifies against the kind table.
        let p = parse_ok("make s equal to a circle with radius 5");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::StructLit { name, fields, .. }, .. } => {
                assert_eq!(name.display(), "circle");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].0.display(), "radius");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn inline_lambda_and_it() {
        // The word run `map things` stays whole at the parser level —
        // multi-word resolution is sema's job (7.0.3). The `using` suffix
        // must land on the call either way.
        let p = parse_ok("make raised equal to map things using taking n giving back n times 2");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                let lam = c.using_arg.as_ref().expect("using argument");
                assert!(matches!(lam.as_ref(), Expr::Lambda { .. }));
            }
            other => panic!("{other:?}"),
        }
        let p = parse_ok("make raised equal to map things using it plus 5");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                let lam = c.using_arg.as_ref().expect("using argument");
                match lam.as_ref() {
                    Expr::Binary { left, .. } => {
                        assert!(matches!(left.as_ref(), Expr::Name { name, .. } if name.display() == "it"));
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn where_suffix_parses_full_expression() {
        // `where` takes an orexpr (R-3): comparisons without parens. The
        // callee word run stays whole (sema splits it).
        let p = parse_ok("make passing equal to keep scores where it is at least 80");
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Call(c), .. } => {
                assert!(c.where_expr.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn block_lambda_parses() {
        let p = parse_ok(
            "make twice equal to a function taking f\n    gives back f of f of 3\n",
        );
        match first_stmt(&p) {
            Stmt::Make { value: Expr::Lambda { params, body, .. }, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].display(), "f");
                match body {
                    LambdaBody::Block(block) => assert_eq!(block.stmts.len(), 1),
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
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
    gives back total divided evenly by size of scores
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
    gives back correct answers divided by total questions
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
    gives back nothing
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
