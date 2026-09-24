// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! Stamp the build profile at compile time so the CLI's version line says
//! what it is (26.1): `lagom 0.1.0 (dev)` vs `(release)`. Also stamp the
//! provenance (LAGOM_BUILD_COMMIT): the git commit this binary was built
//! from, set when CI or a tagged build stamps it (git rev-parse or
//! GITHUB_SHA). A local build keeps its default — the version line prints
//! "built from a local checkout" — so a redistributed binary always names
//! the source it came from.

fn main() {
    let display = match std::env::var("PROFILE").as_deref() {
        Ok("release") => "release",
        _ => "dev",
    };
    println!("cargo:rustc-env=LAGOM_BUILD_PROFILE={display}");

    let commit = std::env::var("LAGOM_BUILD_COMMIT").unwrap_or_default();
    println!("cargo:rustc-env=LAGOM_BUILD_COMMIT={commit}");
}
