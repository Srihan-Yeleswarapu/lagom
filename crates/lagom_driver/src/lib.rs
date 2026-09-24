// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The Lagom driver (M0) — pipeline orchestration (doc 08 §5, 21.5).
//!
//! Three modules, one concern each:
//!
//! - [`pipeline`] — the shared front end (parse → check → HIR → MIR) and
//!   diagnostic rendering; every backend consumes its output.
//! - [`native`] — the native back end: object code, runtime discovery, and
//!   the rustc link step.
//! - [`interp`] — the interpreter back end (run and tests on the MIR).
//!
//! This file is the public face: it re-exports the surface the CLI and the
//! tests use, so the module split is internal structure, not API churn.

mod interp;
mod native;
mod pipeline;
mod repl;

pub use pipeline::{
    frontend, render_diagnostics, render_diagnostics_in, Frontend, FrontendError,
};
pub use lagom_diagnostics::Verbosity;
pub use interp::{
    call_counts, needs_input, run_fe, run_full, run_interpreted, run_tests_interpreted, run_traced,
    run_traced_fe,
};
pub use native::{build_native, compile_object, link, CompiledObject, NativeBuild};
pub use repl::{Session, StepOutcome};
