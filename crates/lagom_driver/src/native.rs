//! The native back end: source → object → executable. Owns everything about
//! producing a binary — codegen, the runtime rlib search order, the rustc
//! link step, and the scratch build directory's lifecycle.

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
    workspace_rlib()
}

/// The bundled copy next to the binary: `<exe dir>/lib/lagom/`. Wins only
/// when it is newer than every runtime source file — a developer running a
/// release binary from inside the workspace must never link a stale bundle.
fn bundled_rlib() -> Option<(PathBuf, PathBuf)> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let lib_dir = exe_dir.join("lib/lagom");
    let rlib = lib_dir.join("liblagom_rt.rlib");
    let rm = mtime(&rlib)?;
    let stale = runtime_sources()
        .iter()
        .any(|s| mtime(s).map(|sm| sm > rm).unwrap_or(false));
    if stale {
        return None;
    }
    Some((rlib, lib_dir.join("deps")))
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
        build_runtime(&workspace, &rt_dir);
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

fn build_runtime(workspace: &Path, target_dir: &Path) {
    // Quiet, best-effort; failures surface as the not-found error above.
    // `CARGO_TARGET_DIR` already names the profile subdirectory
    // (`target/lagom-rt/debug`), and cargo appends its own `debug`/`release`
    // — so point it one level up and let cargo do the layout.
    let parent = target_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| target_dir.to_path_buf());
    let _ = Command::new("cargo")
        .args(["build", "-p", "lagom_rt"])
        .env("CARGO_TARGET_DIR", parent)
        .current_dir(workspace)
        .output();
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
    let status = Command::new("rustc")
        .arg(&shim_path)
        .args(["--edition", "2021"])
        .args(["-C", "extra-filename="])
        .args(["-C", &format!("link-arg={}", obj_path.display())])
        .args(["--extern", &format!("lagom_rt={}", rt_rlib.display())])
        .args(["-L", &format!("dependency={}", deps_dir.display())])
        .args(["-C", "opt-level=2"])
        .args(["-C", "debuginfo=0"])
        .args(if no_pdb {
            ["-C", "link-arg=/DEBUG:NONE"].as_slice()
        } else {
            &[][..]
        })
        .arg("-o")
        .arg(out)
        .status()
        .map_err(|e| format!("running rustc: {e} — is the Rust toolchain on PATH?"))?;
    if !status.success() {
        return Err("linking the runtime failed (rustc exited with an error)".to_string());
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
