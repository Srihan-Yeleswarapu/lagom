// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The starter source `lagom new` writes.

pub const NEW_TEMPLATE: &str = r#"# A brand-new Lagom program.

function greet
    takes text called name
    gives back "Hello, {name}!"

make who equal to ask "Who is it? "
say greet who
"#;
