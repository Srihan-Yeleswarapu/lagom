//! Stamp the cargo profile directory name so the driver finds the runtime
//! rlib built in the *matching* profile (release CLI → release runtime).

fn main() {
    let dir = match std::env::var("PROFILE").as_deref() {
        Ok("release") => "release",
        _ => "debug",
    };
    println!("cargo:rustc-env=LAGOM_RT_PROFILE={dir}");
}
