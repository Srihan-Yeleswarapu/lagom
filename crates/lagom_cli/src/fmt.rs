//! The canonical formatter (00 §26.3): 4-space indentation, zero-config,
//! lossless in meaning (the AST round-trips through parse → format → parse
//! to the same AST modulo spans). Idempotent by construction: the output is
//! produced by one deterministic walk.

use lagom_ast::*;

const INDENT: &str = "    ";

/// Re-escape text for a string literal (7.7's escape set): `"`, `\\`, and
/// the interpolation braces are escapable, so any of them in the value must
/// print escaped or the formatted file would not reparse to the same AST
/// (the formatter's lossless contract). Newlines are string content (G-5's
/// verbatim form), so they print as themselves.
fn escape_text(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' | '\\' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
}

/// Format a whole program.
pub fn format(program: &Program) -> String {
    let mut out = String::new();
    let mut first = true;
    for item in &program.items {
        if !first {
            out.push('\n');
        }
        first = false;
        format_item(&mut out, item);
    }
    out
}

fn format_item(out: &mut String, item: &Item) {
    match item {
        Item::Function(f) => format_function(out, f),
        Item::Structure(s) => {
            out.push_str(&format!("structure {}\n", s.name.display()));
            for field in &s.fields {
                out.push_str(&format!(
                    "{INDENT}has {} of type {}\n",
                    field.name.display(),
                    field.ty.display()
                ));
            }
        }
        Item::Kind(k) => {
            // `kind name ⏎ INDENT { is a variant [with f of type T …] }`
            out.push_str(&format!("kind {}\n", k.name.display()));
            for v in &k.variants {
                out.push_str(&format!("{INDENT}is a {}", v.name.display()));
                for (fname, fty, _) in &v.fields {
                    out.push_str(&format!(" with {} of type {}", fname.display(), fty.display()));
                }
                out.push('\n');
            }
        }
        Item::TypeAlias(a) => {
            // One canonical line: the article spelling is uniform, the target
            // type renders in its 7.10 word style.
            out.push_str(&format!(
                "a type called {} is a {}\n",
                a.name.display(),
                a.ty.display()
            ));
        }
        Item::Test(t) => {
            out.push_str(&format!("test \"{}\"\n", t.name));
            format_block(out, &t.body, 1);
        }
        Item::Use(u) => match &u.imports {
            Some(names) => {
                let list = names
                    .iter()
                    .map(|n| n.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("use {} for {list}\n", u.module.display()));
            }
            None => out.push_str(&format!("use {}\n", u.module.display())),
        },
        Item::Stmt(s) => format_stmt(out, s, 0),
    }
}

fn format_function(out: &mut String, f: &FunctionDecl) {
    out.push_str(&format!("function {}\n", f.name.display()));
    for p in &f.params {
        if matches!(p.ty, TypeExpr::Inferred) {
            out.push_str(&format!("{INDENT}takes {}\n", p.name.display()));
        } else {
            out.push_str(&format!(
                "{INDENT}takes {} called {}\n",
                p.ty.display(),
                p.name.display()
            ));
        }
    }
    if let Some(ty) = &f.returns {
        out.push_str(&format!("{INDENT}returns {}\n", ty.display()));
    }
    if f.can_fail {
        out.push_str(&format!("{INDENT}can fail\n"));
    }
    format_block(out, &f.body, 1);
}

fn format_block(out: &mut String, block: &Block, depth: usize) {
    let pad = INDENT.repeat(depth);
    for s in &block.stmts {
        out.push_str(&pad);
        format_stmt_indented(out, s, depth);
    }
}

/// Statements print with a trailing newline; nested blocks indent deeper.
fn format_stmt(out: &mut String, s: &Stmt, depth: usize) {
    format_stmt_indented(out, s, depth)
}

fn format_stmt_indented(out: &mut String, s: &Stmt, depth: usize) {
    match s {
        Stmt::Make { mutable, name, value, annotation, .. } => {
            out.push_str("make ");
            if *mutable {
                out.push_str("changing ");
            }
            out.push_str(&name.display());
            out.push_str(" equal to ");
            format_expr(out, value, 0, depth);
            if let Some(ty) = annotation {
                out.push_str(&format!(" of type {}", ty.display()));
            }
            out.push('\n');
        }
        Stmt::Set { target, value, .. } => {
            out.push_str("set ");
            format_target(out, target);
            out.push_str(" to ");
            format_expr(out, value, 0, depth);
            out.push('\n');
        }
        Stmt::Change { decrease, target, value, .. } => {
            out.push_str(if *decrease { "decrease " } else { "increase " });
            format_target(out, target);
            out.push_str(" by ");
            format_expr(out, value, 0, depth);
            out.push('\n');
        }
        Stmt::If { branches, otherwise, .. } => {
            for (i, (cond, block)) in branches.iter().enumerate() {
                if i == 0 {
                    out.push_str("if ");
                } else {
                    // A continuation header line carries its own pad: the
                    // caller only indents the statement's first line.
                    out.push_str(&format!("{}otherwise if ", INDENT.repeat(depth)));
                }
                format_expr(out, cond, 0, depth);
                out.push('\n');
                format_block(out, block, depth + 1);
            }
            if let Some(block) = otherwise {
                out.push_str(&format!("{}otherwise\n", INDENT.repeat(depth)));
                format_block(out, block, depth + 1);
            }
        }
        Stmt::Repeat(r) => match r {
            Repeat::Count { times, binding, body, .. } => {
                out.push_str(&format!("repeat {} times", times.0));
                if let Some(b) = binding {
                    out.push_str(&format!(" using {}", b.display()));
                }
                out.push('\n');
                format_block(out, body, depth + 1);
            }
            Repeat::While { cond, body, .. } => {
                out.push_str("repeat while ");
                format_expr(out, cond, 0, depth);
                out.push('\n');
                format_block(out, body, depth + 1);
            }
            Repeat::ForEach { item, index, iter, body, .. } => {
                out.push_str("repeat for each ");
                out.push_str(&item.display());
                if let Some(ix) = index {
                    out.push_str(&format!(", {}", ix.display()));
                }
                out.push_str(" in ");
                format_expr(out, iter, 0, depth);
                out.push('\n');
                format_block(out, body, depth + 1);
            }
        },
        Stmt::Stop { .. } => out.push_str("stop\n"),
        Stmt::Next { .. } => out.push_str("next\n"),
        Stmt::GiveBack { value, .. } => {
            out.push_str("gives back ");
            format_expr(out, value, 0, depth);
            out.push('\n');
        }
        Stmt::FailWith { value, .. } => {
            out.push_str("fail with ");
            format_expr(out, value, 0, depth);
            out.push('\n');
        }
        Stmt::Attempt { expr, tail, .. } => {
            out.push_str("attempt ");
            format_expr(out, expr, 0, depth);
            match tail {
                None => out.push('\n'),
                Some(AttemptTail::Propagate) => out.push_str(" and pass the problem on\n"),
                Some(AttemptTail::IfItFails { then_block, otherwise }) => {
                    out.push_str(" if it fails then\n");
                    format_block(out, then_block, depth + 1);
                    if let Some(block) = otherwise {
                        out.push_str(&format!("{}otherwise\n", INDENT.repeat(depth)));
                        format_block(out, block, depth + 1);
                    }
                }
                Some(AttemptTail::As { name, block, otherwise }) => {
                    out.push_str(&format!(" as {}\n", name.display()));
                    format_block(out, block, depth + 1);
                    if let Some(block) = otherwise {
                        out.push_str(&format!("{}otherwise\n", INDENT.repeat(depth)));
                        format_block(out, block, depth + 1);
                    }
                }
            }
        }
        Stmt::CheckThat { expr, .. } => {
            out.push_str("check that ");
            format_expr(out, expr, 0, depth);
            out.push('\n');
        }
        Stmt::Match { scrutinee, arms, otherwise, .. } => {
            out.push_str("match ");
            format_expr(out, scrutinee, 0, depth);
            out.push('\n');
            for (pattern, block) in arms {
                out.push_str(&format!("{}when ", INDENT.repeat(depth + 1)));
                format_pattern(out, pattern);
                out.push('\n');
                format_block(out, block, depth + 2);
            }
            if let Some(block) = otherwise {
                out.push_str(&format!("{}otherwise\n", INDENT.repeat(depth + 1)));
                format_block(out, block, depth + 2);
            }
        }
        Stmt::ExprStmt { expr, .. } => {
            format_expr(out, expr, 0, depth);
            out.push('\n');
        }
    }
}

/// One `when` arm's pattern in canonical spelling (7.12/7.15).
fn format_pattern(out: &mut String, p: &Pattern) {
    match p {
        Pattern::Literal { value, .. } => match value {
            PatternLiteral::Int(v) => out.push_str(&v.to_string()),
            PatternLiteral::Float(v) => out.push_str(&v.to_string()),
            PatternLiteral::Text(s) => {
                out.push('"');
                escape_text(out, s);
                out.push('"');
            }
            PatternLiteral::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            PatternLiteral::Nothing => out.push_str("nothing"),
        },
        Pattern::Name { name } => out.push_str(&name.display()),
        Pattern::Variant { name, fields, .. } => {
            out.push_str(&format!("a {}", name.display()));
            for (fname, sub) in fields {
                out.push_str(" with ");
                out.push_str(&fname.display());
                // `with radius r` (field + binding) vs `with radius` (the
                // binding is the field) — print the pair only when they differ.
                match sub {
                    Pattern::Name { name: binding } if binding.words == fname.words => {}
                    other => {
                        out.push(' ');
                        format_pattern(out, other);
                    }
                }
            }
        }
        Pattern::Something { inner, .. } => {
            out.push_str("something with value ");
            format_pattern(out, inner);
        }
        Pattern::Pair { first, second, .. } => {
            out.push_str("a pair of ");
            format_pattern(out, first);
            out.push_str(" and ");
            format_pattern(out, second);
        }
        // The parser never produces a wildcard pattern (`otherwise` carries
        // the catch-all arm); print the word that would mean the same thing.
        Pattern::Wildcard => out.push_str("otherwise"),
    }
}

fn format_target(out: &mut String, t: &Target) {
    out.push_str(&t.base.display());
    for a in &t.accessors {
        match a {
            Accessor::At { index, .. } => {
                out.push_str(" at ");
                format_expr(out, index, 0, 0);
            }
            Accessor::Of { field, .. } => {
                out.push_str(&format!(" of {}", field.display()));
            }
        }
    }
}

/// Expression printing with minimal parentheses (the precedence ladder of 7.3).
/// `depth` is the enclosing block's depth: multi-line expressions (block
/// lambdas) indent their bodies one level under their header line.
pub fn format_expr(out: &mut String, e: &Expr, parent_prec: u8, depth: usize) {
    let prec = expr_prec(e);
    let needs_parens = prec < parent_prec;
    if needs_parens {
        out.push('(');
    }
    format_expr_inner(out, e, depth);
    if needs_parens {
        out.push(')');
    }
}

/// Precedence: higher binds tighter (7.3's ladder).
fn expr_prec(e: &Expr) -> u8 {
    match e {
        Expr::Binary { op, .. } => match op {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Equal | BinOp::NotEqual | BinOp::Greater | BinOp::Less | BinOp::AtLeast | BinOp::AtMost => 4,
            BinOp::Add | BinOp::Sub => 5,
            BinOp::Mul | BinOp::Div | BinOp::DivEvenly | BinOp::Rem => 6,
        },
        Expr::Not { .. } => 3,
        Expr::Neg { .. } => 7,
        Expr::Call(_) => 7,
        Expr::AttemptExpr { .. } => 0,
        _ => 8,
    }
}

fn format_expr_inner(out: &mut String, e: &Expr, depth: usize) {
    match e {
        Expr::Int { value, .. } => out.push_str(&value.to_string()),
        Expr::Float { value, .. } => out.push_str(&value.to_string()),
        Expr::Text { value, .. } => {
            out.push('"');
            escape_text(out, value);
            out.push('"');
        }
        Expr::Bool { value, .. } => out.push_str(if *value { "true" } else { "false" }),
        Expr::Nothing { .. } => out.push_str("nothing"),
        Expr::Name { name, .. } => out.push_str(&name.display()),
        Expr::Interp { parts, .. } => {
            out.push('"');
            for p in parts {
                match p {
                    InterpPart::Lit(text) => escape_text(out, text),
                    InterpPart::Expr(inner) => {
                        out.push('{');
                        format_expr_inner(out, inner, depth);
                        out.push('}');
                    }
                }
            }
            out.push('"');
        }
        Expr::Group { inner, .. } => {
            out.push('(');
            format_expr_inner(out, inner, depth);
            out.push(')');
        }
        Expr::Neg { inner, .. } => {
            out.push('-');
            format_expr(out, inner, 7, depth);
        }
        Expr::Not { inner, .. } => {
            out.push_str("not ");
            format_expr(out, inner, 3, depth);
        }
        Expr::Binary { op, left, right, .. } => {
            let p = expr_prec(e);
            format_expr(out, left, p, depth);
            out.push(' ');
            out.push_str(binop_text(*op));
            out.push(' ');
            format_expr(out, right, p + 1, depth);
        }
        Expr::Call(c) => format_call(out, c, depth),
        Expr::ListLit { elements, .. } => {
            out.push_str("a list of ");
            let parts: Vec<String> = elements
                .iter()
                .map(|el| {
                    let mut s = String::new();
                    format_expr(&mut s, el, 0, depth);
                    s
                })
                .collect();
            out.push_str(&parts.join(", "));
        }
        Expr::MapLit { entries, .. } => {
            out.push_str("a map from ");
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_expr(out, k, 0, depth);
                out.push_str(" to ");
                format_expr(out, v, 0, depth);
            }
        }
        Expr::PairLit { first, second, .. } => {
            out.push_str("a pair of ");
            format_expr(out, first, 0, depth);
            out.push_str(" and ");
            format_expr(out, second, 0, depth);
        }
        Expr::StructLit { name, fields, .. } => {
            out.push_str(&format!("a {}", name.display()));
            for (i, (fname, val)) in fields.iter().enumerate() {
                out.push_str(if i == 0 { " with " } else { " and " });
                out.push_str(&fname.display());
                out.push(' ');
                format_expr(out, val, 0, depth);
            }
        }
        Expr::VariantLit { name, fields, .. } => {
            // Variant construction (7.12) prints exactly like construction
            // (`a circle with radius 5`); sema resolved the name.
            out.push_str(&format!("a {}", name.display()));
            for (i, (fname, val)) in fields.iter().enumerate() {
                out.push_str(if i == 0 { " with " } else { " and " });
                out.push_str(&fname.display());
                out.push(' ');
                format_expr(out, val, 0, depth);
            }
        }
        Expr::AttemptExpr { expr, .. } => {
            out.push_str("attempt ");
            format_expr_inner(out, expr, depth);
        }
        Expr::SomeValue { value, .. } => {
            // The option construction (8.5/G-21) prints its canonical form.
            out.push_str("something with value ");
            format_expr(out, value, 0, depth);
        }
        Expr::Lambda { params, body, .. } => {
            out.push_str("a function");
            if !params.is_empty() {
                out.push_str(" taking ");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(" and ");
                    }
                    out.push_str(&p.display());
                }
            }
            match body {
                LambdaBody::Inline(inner) => {
                    out.push_str(" giving back ");
                    format_expr(out, inner, 0, depth);
                }
                LambdaBody::Block(b) => {
                    out.push('\n');
                    format_block(out, b, depth + 1);
                }
            }
        }
    }
}

fn format_call(out: &mut String, c: &CallExpr, depth: usize) {
    out.push_str(&c.callee.display());
    if let Some(first) = &c.first {
        out.push(' ');
        format_expr(out, &first.expr, 0, depth);
    }
    for (prep, arg) in &c.preps {
        out.push(' ');
        out.push_str(match prep {
            Prep::Of => "of",
            Prep::At => "at",
            Prep::From => "from",
            Prep::To => "to",
        });
        out.push(' ');
        format_expr(out, &arg.expr, 0, depth);
    }
    for arg in &c.and_args {
        out.push_str(" and ");
        format_expr(out, &arg.expr, 0, depth);
    }
    for (name, arg) in &c.with_args {
        out.push_str(&format!(" with {} ", name.display()));
        format_expr(out, &arg.expr, 0, depth);
    }
    // The M1 call suffixes round-trip (R-3's fixed suffix order).
    if let Some(lam) = &c.using_arg {
        out.push_str(" using ");
        format_expr(out, lam, 0, depth);
    }
    if let Some(w) = &c.where_expr {
        out.push_str(" where ");
        format_expr(out, w, 0, depth);
    }
}

fn binop_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        // The canonical spelling is the word (D-7: the two division concepts
        // are taught through the words; `/` is the alias).
        BinOp::Div => "divided by",
        BinOp::DivEvenly => "divided evenly by",
        BinOp::Rem => "remainder of",
        BinOp::And => "and",
        BinOp::Or => "or",
        BinOp::Equal => "is equal to",
        BinOp::NotEqual => "is not equal to",
        BinOp::Greater => "is greater than",
        BinOp::Less => "is less than",
        BinOp::AtLeast => "is at least",
        BinOp::AtMost => "is at most",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// fmt's own contract (module doc): parse → format → parse is the same
    /// AST modulo spans, and the output reparses without diagnostics.
    fn assert_roundtrips(src: &str) {
        let (p1, diags) = lagom_parser::parse(src);
        assert!(diags.is_empty(), "source must parse: {diags:?}");
        let formatted = format(&p1);
        let (p2, diags2) = lagom_parser::parse(&formatted);
        assert!(diags2.is_empty(), "formatted output must reparse: {diags2:?}\n{formatted}");
        let again = format(&p2);
        assert_eq!(formatted, again, "fmt must be idempotent");
    }

    /// A `"` inside a string must print escaped, or the formatted file does
    /// not reparse (audit 2026-09-13: csv_parser's `split part and "\""`
    /// formatted to `"""` and died at parse).
    #[test]
    fn text_literals_reparse_after_format() {
        assert_roundtrips("make s equal to \"say \\\"hi\\\" now\"\n");
        assert_roundtrips("make t equal to \"back\\\\slash {not interp}\"\n");
        assert_roundtrips("make u equal to \"tab\tinside\"\n");
    }

    /// Nested `otherwise` headers carry their own indentation (the caller
    /// pads only the statement's first line) — audit 2026-09-13.
    #[test]
    fn nested_otherwise_stays_indented() {
        assert_roundtrips("if true\n    say \"a\"\notherwise\n    if false\n        say \"b\"\n    otherwise\n        say \"c\"\n");
    }
}
