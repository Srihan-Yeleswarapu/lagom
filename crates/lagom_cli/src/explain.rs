//! `lagom explain <code>` — the teaching write-ups (00 §26.1/§26.2: docs in
//! the compiler, so explanations ship with the version that emits them).
//! M0 covers the concepts every diagnostic links to; the code index below is
//! the full M0 diagnostic surface.

/// The write-up for a code, if one exists.
pub fn lookup(code: &str) -> Option<String> {
    let body = match code {
        "E0330" => (
            "A name can only mean one thing.",
            "You made the same name twice. If you need a second value, pick a different name; if you want to change the value, make the first one `changing` and use `set`.",
            "make score equal to 10\nset score to 20   # needs `make changing score equal to 10` first",
            "Variables and names (7.2)",
        ),
        "E0331" => (
            "Two names that start the same words are ambiguous.",
            "Lagom reads names greedily, so `top` and `top score` cannot both exist — reading `top score` could mean either.",
            "rename one of the two names so neither is a word-prefix of the other",
            "Multi-word names (7.0.3)",
        ),
        "E0332" => (
            "An error value must be handled where it is produced.",
            "A `can fail` call produces an error value. Wrap it in `attempt`, or (inside another `can fail` function) write `attempt … and pass the problem on`.",
            "attempt divide 10 and 0 if it fails then\n    say \"cannot divide\"\notherwise\n    say result",
            "Errors are values (13.1)",
        ),
        "E0333" => (
            "A function that promises a value must give one back.",
            "You wrote `returns …`, so every path through the function must end with `give back <value>`.",
            "give back the value on every path through the function",
            "Functions (7.8)",
        ),
        "E0334" => (
            "Only a `changing` variable can be changed.",
            "`make x equal to …` creates an immutable binding. Use `make changing x equal to …` when the value will change.",
            "make changing score equal to 0\nincrease score by 5",
            "Immutable by default (7.2, D-5)",
        ),
        "E0335" => (
            "`stop` and `next` belong to loops.",
            "`stop` leaves the nearest `repeat`; `next` jumps to its next turn. Outside a loop they have nothing to act on.",
            "repeat 10 times using i\n    stop",
            "Loops (7.5)",
        ),
        "E0336" => (
            "`give back` belongs to a function.",
            "Only a function body can return a value to its caller.",
            "move the `give back` inside a function",
            "Functions (7.8)",
        ),
        "E0337" => (
            "`fail with` belongs in a `can fail` function.",
            "Failing is a declared capability: the function signature must carry `can fail` so callers know an error value can come out.",
            "function divide\n    takes number called top\n    takes number called bottom\n    returns a decimal\n    can fail\n    …",
            "The error model (13.1)",
        ),
        "E0338" => (
            "You can only repeat for each … in a list (or text).",
            "`repeat for each item in …` walks a collection. Numbers have nothing to walk.",
            "repeat for each item in a list of 1, 2, 3\n    say item",
            "Loops and lists (7.5, 7.6)",
        ),
        "E0339" => (
            "Immutable values cannot be assigned to.",
            "`set` (and `increase`/`decrease`) only work on `changing` bindings.",
            "make changing score equal to 0\nset score to 5",
            "Immutable by default (7.2)",
        ),
        "E0340" => (
            "That module does not exist.",
            "M0 ships `standard` (always in scope), `math`, and `random`. Modules come from files in your project (one file = one module).",
            "use math for square root",
            "Modules (7.13)",
        ),
        "E0341" => (
            "Two things in one module cannot share a name.",
            "Function and structure names are module-wide; duplicates are ambiguous.",
            "rename one of them",
            "Modules (7.13)",
        ),
        "E0342" => (
            "That name is not a function, a variable, or a field.",
            "Nothing with this name is in scope here. Check the spelling, or add the `use` that brings it in.",
            "use math for square root\nsay square root of 16",
            "Names and scope (7.0.3, 7.13)",
        ),
        "E0343" => (
            "A field access needs a structure value.",
            "`x of p` reads the field `x` from the structure `p`. If `p` is not a structure, there is nothing to read.",
            "make p equal to a player with name \"bo\" and score 0\nsay name of p",
            "Structures (7.11)",
        ),
        "E0344" => (
            "That name is not defined.",
            "Nothing with this name exists at this point. Check the spelling — Lagom catches typos like `scoer` for `score`.",
            "define it first, or fix the spelling",
            "Names and scope (7.0.3)",
        ),
        "E0345" => (
            "A structure field must be given a value when you build it.",
            "`with` construction must give every declared field a value.",
            "a player with name \"bo\" and score 0",
            "Structures (7.11)",
        ),
        "E0346" => (
            "A list holds one kind of value.",
            "Every element of a list has the same type — mixing `1` and `\"a\"` has no single type to be.",
            "a list of 1, 2, 3   — or —   a list of \"a\", \"b\"",
            "Lists and types (7.6, 8)",
        ),
        "E0347" => (
            "A map's keys must all be the same type (and so must its values).",
            "A map is one key type to one value type.",
            "a map from \"ana\" to 11, \"bo\" to 12",
            "Maps (7.6)",
        ),
        "E0348" => (
            "That structure has no field with that name.",
            "Check the `structure` declaration for the field's exact name.",
            "structure player\n    has name of type text",
            "Structures (7.11)",
        ),
        "E0349" => (
            "You are missing a field in this construction.",
            "`with` construction must provide every field the structure declares.",
            "a player with name \"bo\" and score 0",
            "Structures (7.11)",
        ),
        "E0353" => (
            "The math module's functions need the right kinds of values.",
            "`square root of n` takes a number or decimal and gives a decimal; `floor of n` takes a decimal and gives a number.",
            "use math for square root\nsay square root of 16",
            "The math module (docs/13 S-7/G-9)",
        ),
        "E0354" => (
            "This call needs a different number of arguments.",
            "Check the function's `takes` clauses — one value per clause, in order.",
            "greet with name \"bo\"   — for —   function greet / takes text called name",
            "Functions (7.8)",
        ),
        "E0355" => (
            "A condition must be a boolean.",
            "`if` (and `repeat while`) needs a value that is `true` or `false`. Use a comparison, not a bare value.",
            "if score is at least 50\n    say \"pass\"",
            "Conditions and booleans (7.3, 7.4)",
        ),
        "E0356" => (
            "Only numbers and decimals can be ordered.",
            "Words like `is greater than` compare amounts. Compare text with `is equal to`.",
            "if count is greater than 3\n    …",
            "Comparisons (7.3)",
        ),
        "E0357" => (
            "You cannot do arithmetic on text.",
            "`+`/`plus` works on numbers (or decimals). To join text, use interpolation.",
            "say \"Hello, {name}!\"",
            "Text and interpolation (7.7)",
        ),
        "E0358" => (
            "This expression has to be a boolean (or produce one).",
            "`and`/`or`/`not`, `check that`, and conditions all work on `true`/`false` values.",
            "check that 1 is equal to 1",
            "Booleans (7.3, S-4)",
        ),
        "E0359" => (
            "This conversion needs text to convert from.",
            "`number from x` parses text into a number; give it a text value.",
            "make answer equal to ask \"pick a number: \"\nnumber from answer",
            "Text and number conversion (D-39)",
        ),
        "E0360" => (
            "This value is not the type the annotation promises.",
            "Values have one type. If you promised `number`, give it a number — or drop the annotation and let Lagom infer it.",
            "make n equal to 5 of type number",
            "Types and inference (7.10, 8.1)",
        ),
        "E0302" => (
            "A `can fail` call must be wrapped in an attempt.",
            "Calling a function that can fail hands you an error value you must handle — with `attempt … if it fails then … otherwise …`, `attempt … as problem …`, or `attempt … and pass the problem on` inside another `can fail` function.",
            "attempt number from answer if it fails then\n    say \"that was not a number!\"\notherwise\n    say result",
            "Errors are values (13.1)",
        ),
        "E0101" => (
            "The lexer could not read this character.",
            "Every character must belong to the language: identifiers, the operator symbols, or text.",
            "check for a stray or unsupported character",
            "Lexical rules (7.0)",
        ),
        _ => return None,
    };
    let (what, why, fix, concept) = body;
    Some(format!(
        "{code} — {what}\n\n{why}\n\nTo fix, write:\n\n    {fix}\n\nConcept: {concept}\n"
    ))
}

/// Every code the M0 compiler emits, grouped by stage (codes are stable
/// identifiers: E01xx lexer, E02xx parser, E03xx semantics).
pub fn code_index() -> String {
    let mut out = String::new();
    out.push_str("  lexer   E0101–E010x: characters and text that cannot be read\n");
    out.push_str("  parser  E0201–E020x: lines that do not fit the grammar\n");
    out.push_str("  sema    E03xx: names, types, capabilities, and structure rules\n");
    out.push_str("\nTry `lagom explain E0330` (duplicate name), `lagom explain E0302` (unhandled\nfailure), or `lagom explain E0344` (unknown name).\n");
    out
}
