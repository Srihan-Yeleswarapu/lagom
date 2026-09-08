//! The starter source `lagom new` writes.

pub const NEW_TEMPLATE: &str = r#"# A brand-new Lagom program.

function greet
    takes text called name
    give back "Hello, {name}!"

make who equal to ask "Who is it? "
say greet who
"#;
