//! The native back end: source → object → executable. Owns everything about
//! producing a binary — codegen, the runtime rlib search order (including
//! the rustc-version stamp that keeps shipped rlibs honest), the rustc link
//! step, and the scratch build directory's lifecycle.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pipeline::{frontend, FrontendError};

/// The compiled object plus the front-end facts `lagom run` needs to speak
/// honestly about a program with nothing to execute (test-only files).
pub struct CompiledObject {
    pub object: Vec<u8>,
    /// Whether the program has a script body — the thing `run` executes.
    pub has_entry: bool,
    /// How many `test` blocks the program declares.
    pub test_count: usize,
}

/// Compile source all the way to a native object file.
///
/// `no_pdb` controls whether debug-info emission is discarded at the object
/// level. The default for user-facing builds is `true`: do not produce a
/// `.pdb` unless the caller passes `--pdb`.
pub fn compile_object(src: &str, dev: bool, no_pdb: bool) -> Result<CompiledObject, FrontendError> {
    let fe = frontend(src)?;
    // LIR bakes 1-based line numbers into traps from the source text.
    lagom_lir::set_source(src);
    let lir = lagom_lir::lower(&fe.mir, dev);
    let object = lagom_codegen::compile(&lir, src, dev, no_pdb).map_err(|e| FrontendError::Internal {
        stage: "codegen",
        message: e,
    })?;
    Ok(CompiledObject { object, has_entry: fe.has_entry, test_count: fe.test_count })
}

/// The rustc shim that becomes a Lagom binary's `main`: init the runtime,
/// run the program, exit with its code (23.5: the driver drives the
/// platform linker through rustc).
const SHIM: &str = r#"
fn main() {
    let code = lagom_rt::__lagom_start();
    std::process::exit(code);
}
"#;

// ---------------------------------------------------------------------------
// The rustc-version stamp
// ---------------------------------------------------------------------------

/// Crate metadata is rustc-version-specific: linking an rlib built by a
/// different rustc dies with E0514. A package built by CI's rustc can never
/// link under the user's rustc unless the versions match — so every shipped
/// (or refreshed) bundle carries `lib/lagom/rustc.txt`, and a bundle from a
/// different rustc is treated as unusable rather than mislinked.
const RUSTC_STAMP: &str = "rustc.txt";

/// The exact compiler a bundle must have been built by to link here.
fn local_rustc() -> Option<String> {
    let out = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.trim().to_string())
}

/// Locate the Lagom runtime rlib, building it in an isolated target
/// directory when missing or stale. `LAGOM_RT_LIB` overrides — the escape
/// hatch for installed builds of the CLI.
///
/// Why an isolated target dir: the workspace's shared `deps/` can hold
/// several `liblagom_mir-*.rlib` variants (test units, feature sets); rustc
/// resolving the *wrong* one rejects the link (E0460). A dedicated
/// `CARGO_TARGET_DIR` holds exactly one consistent closure — `lagom_rt` and
/// everything beneath it — so resolution is deterministic. Freshness is a
/// correctness requirement: a stale rlib's dependency metadata mismatches
/// after a source edit, so the runtime is rebuilt when its sources are
/// newer than the artifact.
///
/// The search order: `LAGOM_RT_LIB` → the bundled copy next to the binary
/// (`<exe dir>/lib/lagom/` — the installed-build story, §26.1) → the
/// workspace's isolated target dir (the developer story, rebuilt on edit).
///
/// The bundled copy is trusted only when it was built by *this* rustc (see
/// the stamp above). On a mismatch, a workspace rebuild is attempted; a
/// toolchain-free install falls back to the snapshot sources shipped in
/// `lib/lagom/src/` (dependency-free, built once with plain rustc), then
/// the bundle's own rlib as a last resort (it fails honestly with E0514 if
/// genuinely incompatible).
fn runtime_rlib() -> Result<(PathBuf, PathBuf), String> {
    if let Ok(path) = std::env::var("LAGOM_RT_LIB") {
        let p = PathBuf::from(path);
        if !p.exists() {
            return Err(format!(
                "LAGOM_RT_LIB points at {}, which does not exist.",
                p.display()
            ));
        }
        let deps = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        return Ok((p, deps));
    }
    if let Some(found) = bundled_rlib() {
        return Ok(found);
    }
    // No usable bundle: try the workspace rebuild (developer story), and if
    // that is impossible, fall back to building the package's snapshot
    // sources with plain rustc (the toolchain-free install story).
    let snapshot_available = snapshot_fallback_available();
    match workspace_rlib() {
        Ok(found) => Ok(found),
        Err(e) => {
            if snapshot_available {
                if let Some((rlib, deps)) = snapshot_rlib() {
                    if build_runtime_from_source(&rlib).is_ok() {
                        return Ok((rlib, deps));
                    }
                }
            }
            Err(e)
        }
    }
}

/// The bundled copies next to the binary, in preference order:
///
/// 1. `lib/lagom/built/<profile>/` — a runtime **self-built** from the
///    package's snapshot sources by *this* rustc (its own stamp).
/// 2. `lib/lagom/` — the rlib **shipped** with the package (its stamp in
///    `lib/lagom/rustc.txt`).
///
/// A candidate wins only when its stamp matches the local rustc (rlibs are
/// rustc-version-specific; see the stamp note above) and — for the shipped
/// copy — it is newer than every runtime source file, so a developer
/// running a release binary from inside the workspace never links a stale
/// bundle.
fn bundled_rlib() -> Option<(PathBuf, PathBuf)> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let lib_dir = exe_dir.join("lib/lagom");
    let local = local_rustc();
    let stamped_for_this_rustc = |stamp_path: &Path| {
        std::fs::read_to_string(stamp_path)
            .map(|s| Some(s.trim()) == local.as_deref())
            .unwrap_or(false)
    };
    // 1. The self-built snapshot copy.
    let built = lib_dir.join("built").join(env!("LAGOM_RT_PROFILE"));
    let built_rlib = built.join("liblagom_rt.rlib");
    if built_rlib.exists() && stamped_for_this_rustc(&built.join(RUSTC_STAMP)) {
        if !newer_sources_exist(&built_rlib) {
            return Some((built_rlib, built));
        }
    }
    // 2. The shipped copy.
    let rlib = lib_dir.join("liblagom_rt.rlib");
    if rlib.exists() && stamped_for_this_rustc(&lib_dir.join(RUSTC_STAMP)) {
        if !newer_sources_exist(&rlib) {
            return Some((rlib, lib_dir.join("deps")));
        }
    }
    None
}

/// Any runtime source file newer than `rlib` (sources of an installed
/// package don't exist, so an installed build is never "stale").
fn newer_sources_exist(rlib: &Path) -> bool {
    let rm = match mtime(rlib) {
        Some(rm) => rm,
        None => return true,
    };
    runtime_sources()
        .iter()
        .any(|s| mtime(s).map(|sm| sm > rm).unwrap_or(false))
}

/// Whether the snapshot sources shipped inside a package exist — the
/// condition under which the rustc-only rebuild fallback can run.
fn snapshot_fallback_available() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("lib/lagom/src/lagom_rt")))
        .map(|p| p.is_dir())
        .unwrap_or(false)
}

/// The workspace's isolated target dir, rebuilt when its sources are newer
/// than the artifact (the developer story).
fn workspace_rlib() -> Result<(PathBuf, PathBuf), String> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest).join("../..");
    let rt_dir = workspace.join(format!("target/lagom-rt/{}", env!("LAGOM_RT_PROFILE")));
    let rlib = rt_dir.join("liblagom_rt.rlib");
    let stale = mtime(&rlib)
        .map(|rm| runtime_sources().iter().any(|s| mtime(s).map(|sm| sm > rm).unwrap_or(true)))
        .unwrap_or(true);
    if !rlib.exists() || stale {
        build_runtime(&workspace, env!("LAGOM_RT_PROFILE"), &rt_dir, &[])?;
    }
    if rlib.exists() {
        return Ok((rlib, rt_dir.join("deps")));
    }
    Err(
        "the Lagom runtime library could not be built (cargo build -p lagom_rt failed). \
         Run `cargo build` in the workspace to see the error, or set LAGOM_RT_LIB."
            .to_string(),
    )
}

/// Last-resort rebuild source: the runtime snapshot shipped inside a
/// package at `lib/lagom/src/` (the six dependency-free crates). The chain
/// builds into `<bundled lib>/built/<profile>/`; the bundle stamp written
/// on success routes later runs through the normal bundled path.
fn snapshot_rlib() -> Option<(PathBuf, PathBuf)> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let lib_dir = exe_dir.join("lib/lagom");
    if !lib_dir.join("src").join("lagom_rt").is_dir() {
        return None;
    }
    let out_dir = lib_dir.join("built").join(env!("LAGOM_RT_PROFILE"));
    std::fs::create_dir_all(&out_dir).ok()?;
    Some((out_dir.join("liblagom_rt.rlib"), out_dir.clone()))
}

/// Compile one snapshot crate into `out_dir`, naming every previously
/// built chain crate with an explicit `--extern` (2018+ edition resolution
/// requires it — `-L dependency` alone does not bind names).
fn compile_snapshot_crate(
    name: &str,
    src_dir: &Path,
    out_dir: &Path,
    externs: &[(&str, &Path)],
) -> Result<(), ()> {
    let mut cmd = Command::new("rustc");
    cmd.arg("--edition")
        .arg("2021")
        .args(["--crate-name", name])
        .args(["--crate-type", "rlib"])
        .arg("-O")
        .args(["--out-dir", &out_dir.display().to_string()])
        .args(["-L", &format!("dependency={}", out_dir.display())]);
    for (dep, path) in externs {
        cmd.args(["--extern", &format!("{dep}={}", path.display())]);
    }
    let out = cmd.arg(src_dir.join("lib.rs")).output().map_err(|_| ())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(())
    }
}

/// Build the full snapshot chain diagnostics → ast → sema → hir → mir → rt
/// with plain rustc (no cargo), each crate compiled against the previous
/// ones' outputs. `src_root/<crate>/lib.rs` are the sources; rlibs land in
/// `out_dir`. Every crate in the runtime tree is dependency-free.
fn build_snapshot_chain(src_root: &Path, out_dir: &Path) -> Result<(), ()> {
    const CHAIN: [&str; 6] =
        ["lagom_diagnostics", "lagom_ast", "lagom_sema", "lagom_hir", "lagom_mir", "lagom_rt"];
    let mut built: Vec<(&str, PathBuf)> = Vec::new();
    for name in CHAIN {
        let externs: Vec<(&str, &Path)> =
            built.iter().map(|(n, p)| (*n, p.as_path())).collect();
        compile_snapshot_crate(name, &src_root.join(name), out_dir, &externs)?;
        built.push((name, out_dir.join(format!("lib{name}.rlib"))));
    }
    Ok(())
}

/// Rebuild the runtime from the package's snapshot sources into the
/// `built/<profile>/` directory and stamp it so later runs take the normal
/// bundle path.
fn build_runtime_from_source(rt_rlib: &Path) -> Result<(), ()> {
    // `rt_rlib` = lib/lagom/built/<profile>/liblagom_rt.rlib, so the
    // snapshot sources live two levels up: lib/lagom/src/.
    let out_dir = rt_rlib.parent().ok_or(())?;
    let src_root = out_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.join("src").canonicalize().ok())
        .ok_or(())?;
    build_snapshot_chain(&src_root, out_dir)?;
    if let Some(host) = local_rustc() {
        let _ = std::fs::write(out_dir.join(RUSTC_STAMP), host);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The snapshot fallback's whole premise: the six runtime-tree crates
    /// build with plain rustc — no cargo, no registry, nothing else — and
    /// the result links against the shim exactly like a cargo-built rlib.
    /// This is what lets a downloaded package work under a rustc that did
    /// not build it (the CI install-test pins the full flow).
    #[test]
    fn snapshot_chain_builds_and_links_the_shim() {
        let base = std::env::temp_dir().join(format!(
            "lagom-snapshot-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let src_root = base.join("src");
        let out_dir = base.join("out");
        std::fs::create_dir_all(&out_dir).unwrap();
        for name in [
            "lagom_diagnostics",
            "lagom_ast",
            "lagom_sema",
            "lagom_hir",
            "lagom_mir",
            "lagom_rt",
        ] {
            let dir = src_root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let manifest = env!("CARGO_MANIFEST_DIR");
            let src = Path::new(manifest)
                .join(format!("../{name}/src/lib.rs"));
            std::fs::copy(&src, dir.join("lib.rs")).unwrap();
        }
        build_snapshot_chain(&src_root, &out_dir).expect("snapshot chain builds");
        assert!(out_dir.join("liblagom_rt.rlib").exists());

        // The proof: link the production shim against the snapshot rlibs.
        // A real build's `lagom_main` comes from the compiled object; here a
        // stub defines it so the link resolves exactly the same symbols.
        let dir = base.join("link");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("lagom_shim.rs"),
            format!("{SHIM}\n#[no_mangle]\npub extern \"C\" fn lagom_main() -> i64 {{ 0 }}\n"),
        )
        .unwrap();
        let output = Command::new("rustc")
            .arg(dir.join("lagom_shim.rs"))
            .args(["--edition", "2021"])
            .args(["-C", "extra-filename="])
            .args([
                "--extern",
                &format!("lagom_rt={}", out_dir.join("liblagom_rt.rlib").display()),
            ])
            .args(["-L", &format!("dependency={}", out_dir.display())])
            .arg("-o")
            .arg(dir.join("shim-test"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "shim links against the snapshot chain: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// Runtime source files whose edits invalidate any prebuilt rlib.
fn runtime_sources() -> [PathBuf; 2] {
    let manifest = env!("CARGO_MANIFEST_DIR");
    [
        Path::new(manifest).join("../lagom_rt/src/lib.rs"),
        Path::new(manifest).join("../lagom_mir/src/lib.rs"),
    ]
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Build the runtime rlib in the *matching* profile — a release `lagom`
/// binary must never be handed a debug rlib, and vice versa. The build is
/// quiet but not blind: on failure the compiler's own message surfaces in
/// the driver error, so a broken toolchain is diagnosable from the CLI
/// error alone. After a successful build the bundled copy next to the
/// binary (`<exe dir>/lib/lagom/`, §26.1) is refreshed best-effort, so an
/// installed `lagom` keeps working after the source checkout is deleted.
fn build_runtime(
    workspace: &Path,
    profile: &str,
    target_dir: &Path,
    toolchain: &[&str],
) -> Result<(), String> {
    // `CARGO_TARGET_DIR` names the profile subdirectory's parent and cargo
    // appends its own `debug`/`release` — point it one level up.
    let parent = target_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| target_dir.to_path_buf());
    let mut cmd = Command::new("cargo");
    cmd.args(toolchain);
    cmd.args(["build", "-p", "lagom_rt"]);
    if profile == "release" {
        cmd.arg("--release");
    }
    let output = cmd
        .env("CARGO_TARGET_DIR", parent)
        .current_dir(workspace)
        .output()
        .map_err(|e| format!("could not run cargo to build the runtime: {e}"))?;
    if !output.status.success() {
        let tail = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = tail.lines().rev().take(6).collect();
        let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(format!(
            "the Lagom runtime library could not be built (cargo build -p lagom_rt failed):\n{tail}"
        ));
    }
    refresh_bundle(workspace, target_dir);
    Ok(())
}

/// Copy a freshly built runtime into the bundle next to this binary,
/// best-effort: the workspace build keeps working if the exe dir is not
/// writable or the layout is unexpected.
fn refresh_bundle(workspace: &Path, rt_dir: &Path) {
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    else {
        return;
    };
    // Never write a bundle inside the workspace's target tree — target dirs
    // participate in link resolution (`-L dependency=…`) and a copy landing
    // there races concurrently-linking rustc processes (they mmap the rlib
    // mid-write). This is exactly where test binaries run from.
    if exe_dir.starts_with(workspace.join("target")) {
        return;
    }
    let bundle_dir = exe_dir.join("lib/lagom");
    let Ok(_) = std::fs::create_dir_all(bundle_dir.join("deps")) else {
        return;
    };
    // Atomic per file: copy to a temp name, then rename over the target, so
    // a concurrent reader never sees a partial rlib.
    let bundle_file = |src: &Path, dst: &Path| {
        let tmp = dst.with_extension("rlib.tmp");
        if std::fs::copy(src, &tmp).is_ok() {
            let _ = std::fs::rename(&tmp, dst);
        }
        let _ = std::fs::remove_file(&tmp);
    };
    bundle_file(&rt_dir.join("liblagom_rt.rlib"), &bundle_dir.join("liblagom_rt.rlib"));
    if let Ok(entries) = std::fs::read_dir(rt_dir.join("deps")) {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "rlib") {
                bundle_file(&entry.path(), &bundle_dir.join("deps").join(entry.file_name()));
            }
        }
    }
    // Stamp the bundle with this rustc: a shipped rlib only links under the
    // rustc that built it, and the stamp makes that checkable in one read.
    if let Some(host) = local_rustc() {
        let _ = std::fs::write(bundle_dir.join(RUSTC_STAMP), host);
    }
}

/// Link a Lagom object into an executable. `build_dir` receives the shim
/// and the object; the result is the executable path.
pub fn link(build_dir: &Path, object: &[u8], out: &Path, no_pdb: bool) -> Result<PathBuf, String> {
    std::fs::create_dir_all(build_dir).map_err(|e| format!("build dir: {e}"))?;
    // The `-o` path's parent may not exist yet (`lagom build -o out/x`).
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    let obj_path = build_dir.join("lagom_program.o");
    let shim_path = build_dir.join("lagom_shim.rs");
    std::fs::write(&obj_path, object).map_err(|e| format!("object: {e}"))?;
    std::fs::write(&shim_path, SHIM).map_err(|e| format!("shim: {e}"))?;

    let (rt_rlib, deps_dir) = runtime_rlib()?;
    let output = Command::new("rustc")
        .arg(&shim_path)
        .args(["--edition", "2021"])
        .args(["-C", "extra-filename="])
        .args(["-C", &format!("link-arg={}", obj_path.display())])
        .args(["--extern", &format!("lagom_rt={}", rt_rlib.display())])
        .args(["-L", &format!("dependency={}", deps_dir.display())])
        .args(["-C", "opt-level=2"])
        .args(["-C", "debuginfo=0"])
        .args(if no_pdb && cfg!(windows) {
            ["-C", "link-arg=/DEBUG:NONE"].as_slice()
        } else {
            &[][..]
        })
        .arg("-o")
        .arg(out)
        .output()
        .map_err(|e| format!("running rustc: {e} — is the Rust toolchain on PATH?"))?;
    if !output.status.success() {
        // rustc prints the real reason (E0514 for a version-mismatched
        // rlib, E0460 for stale metadata); surface its last lines so the
        // CLI error alone is diagnosable.
        let tail = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = tail.lines().rev().take(6).collect();
        let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(format!(
            "linking the runtime failed (rustc exited with an error):\n{tail}\
             \nIf this says the runtime was built by a different version of \
             rustc, delete the bundled `lib/lagom` directory's `built` folder \
             (or reinstall the package) and run again — the runtime rebuilds \
             for your toolchain. `LAGOM_RT_LIB` overrides the runtime rlib."
        ));
    }
    Ok(out.to_path_buf())
}

/// A prepared native build: the object plus a scratch directory to link in.
/// The directory is removed when the build is done (Drop), so repeated
/// builds do not accumulate scratch dirs in the system temp directory.
pub struct NativeBuild {
    pub dir: PathBuf,
}

impl NativeBuild {
    /// A unique scratch directory for one build (never collides).
    pub fn new(label: &str) -> Result<NativeBuild, String> {
        let base = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = base.join(format!("lagom-build-{label}-{unique}"));
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
        Ok(NativeBuild { dir })
    }
}

impl Drop for NativeBuild {
    fn drop(&mut self) {
        // Best-effort: a failed removal only leaves a temp dir behind.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Build a native executable from Lagom source.
/// Returns the path of the produced binary.
///
/// `no_pdb` controls whether debug-info emission is discarded at the object
/// level. The default for user-facing builds is `true`: `lagom build` does
/// not create a `.pdb` unless the user passes `--pdb`.
pub fn build_native(src: &str, out: &Path, dev: bool, no_pdb: bool) -> Result<PathBuf, FrontendError> {
    let compiled = compile_object(src, dev, no_pdb)?;
    let object = compiled.object;
    let label = out
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("program")
        .to_string();
    let dir = NativeBuild::new(&label)
        .map_err(|e| FrontendError::Tool(format!("preparing the build: {e}")))?;
    link(&dir.dir, &object, out, no_pdb).map_err(FrontendError::Tool)
}
