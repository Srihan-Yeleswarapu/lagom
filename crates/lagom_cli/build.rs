//! Stamp the build profile at compile time so the CLI's version line says
//! what it is (26.1): `lagom 0.1.0 (dev)` vs `(release)`.

fn main() {
    let display = match std::env::var("PROFILE").as_deref() {
        Ok("release") => "release",
        _ => "dev",
    };
    println!("cargo:rustc-env=LAGOM_BUILD_PROFILE={display}");
}
