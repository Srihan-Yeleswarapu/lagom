//! `lagom explain` — the teaching write-up for an error or lesson code.

use super::args::{read_source, render_failure, CliError, CliResult};
use super::explain;
use lagom_driver::FrontendError;
use lagom_diagnostics::SourceFile;
use std::path::PathBuf;

pub fn cmd_explain(rest: &[String]) -> CliResult {
    let Some(code) = rest.first() else {
        println!(
            "usage: lagom explain <code> [file] — every diagnostic names its code, e.g. `lagom explain E0344`.\n\nA file argument opens the page with that code's real instances in your\nprogram: each occurrence rendered with its span, note, and the context\nsuggests the compiler computed for it (did-you-mean, field tables,\nconstraint lists). The static page alone is the general rule; the\nfile section is the code's own context.\n\nAll codes:\n{}",
            explain::code_index()
        );
        return Ok(());
    };
    // Runtime lessons (R00n) and compiler codes (E0nnn) share one command:
    // the LOM failure report names both kinds (26.5's answer 8).
    let writeup = explain::lookup(code)
        .or_else(|| explain::runtime_lesson(code));
    match writeup {
        Some(writeup) => print!("{writeup}"),
        None => {
            return Err(CliError::Message(format!(
                "no write-up for `{code}` yet.\n\nKnown codes:\n{}",
                explain::code_index()
            )))
        }
    }
    // The context pass: with a file argument, the compiler runs on the file
    // and every firing of THIS code is rendered below the page — the general
    // rule then meets the writer's program (doc 09's ledger: the page ships
    // the concept, the diagnostic instance ships the context).
    if let Some(path) = rest
        .get(1)
        .filter(|p| !p.starts_with('-'))
        .map(PathBuf::from)
    {
        if !path.is_file() {
            return Err(CliError::Message(format!(
                "there is no file called `{}` — the context pass needs an existing source file.",
                path.display()
            )));
        }
        let src = read_source(&path)?;
        let file_name = path.display().to_string();
        match lagom_driver::frontend(&src) {
            Ok(_) => {
                println!(
                    "\nIn {}: `{code}` does not fire — the file checks out clean.\n",
                    file_name
                );
            }
            Err(FrontendError::Diagnostics(list)) => {
                let hits: Vec<&lagom_diagnostics::Diagnostic> =
                    list.iter().filter(|d| d.code == code).collect();
                if hits.is_empty() {
                    println!(
                        "\nIn {}: `{code}` does not fire today. Other diagnostics do — run `lagom check {}` for the list.\n",
                        file_name, file_name
                    );
                } else {
                    println!("\n---------- `{code}` in {}: {} occurrence{} ----------\n", file_name, hits.len(), if hits.len() == 1 { "" } else { "s" });
                    let file = SourceFile::new(&file_name, &src);
                    for d in hits {
                        print!(
                            "{}",
                            lagom_diagnostics::render_student(&file, d)
                        );
                    }
                }
            }
            Err(other) => return Err(render_failure(&path, &src, &other)),
        }
    }
    Ok(())
}
