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

pub use pipeline::{frontend, render_diagnostics, Frontend, FrontendError};
pub use interp::{needs_input, run_interpreted, run_tests_interpreted};
pub use native::{build_native, compile_object, link, CompiledObject, NativeBuild};
