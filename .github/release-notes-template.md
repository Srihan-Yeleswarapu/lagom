# Lagom __VERSION__

One binary plus a bundled runtime. **No Rust toolchain, no source checkout** — download the file for your OS, unpack, run.

| File | For |
|---|---|
| `lagom-Windows.zip` | Windows 10/11 (x64) |
| `lagom-Linux.tar.gz` | Linux (x64) |
| `lagom-macOS.tar.gz` | macOS (Apple Silicon) |

`sha256sums.txt` lets you verify what you downloaded (see below).

**The one rule:** keep `lagom` (or `lagom.exe`) and its `lib` folder together, wherever you unpack — the binary finds its bundled runtime next to itself.

**Requirements:** none to run `check`, `fmt`, `play`, `test`, and `run --trace` (the interpreter needs no toolchain). The first native `lagom run` / `lagom build` rebuilds the bundled runtime once, which needs any Rust toolchain; after that one-time rebuild, no toolchain is needed. macOS packages are Apple Silicon; on an Intel Mac, [build from source](https://github.com/Srihan-Yeleswarapu/lagom#from-source-any-os).

**New in __VERSION__ (M2 — intelligence and tooling):** diagnostics that explain *why* a fix works, tasks and channels, generic types, a REPL, doc comments that turn into checked examples, packages, and a profiler. Details at the bottom.

---

## Install — Windows

1. Unpack the zip somewhere stable, e.g. `C:\Users\you\lagom` (a folder you'll keep, not the Downloads pile).
2. Try it immediately — open PowerShell **in that folder**:
   ```
   .\lagom.exe version
   ```
   → `lagom __VERSION__ (release)`
3. To type plain `lagom` from anywhere, add the folder to PATH (once):
   - Press **Win**, type **environment**, open **Edit environment variables for your account**
   - Under *User variables*, select **Path** → **Edit…** → **New** → paste the folder path → **OK**
   - **Close and reopen your terminal** (open ones don't see the new PATH)
   - ⚠️ Skip internet guides that say `setx` — it can truncate an existing PATH. The GUI way above can't.

**Day one:**
```
.\lagom.exe new hello
cd hello
..\lagom.exe run
```
Type your name when it asks: `Who is it? Hello, you!` (Once PATH is set, plain `lagom new hello` / `lagom run` works from anywhere.)

## Install — macOS

**The easy way** (from a clone of the repo): `sh install.sh` — it downloads the right package for your CPU, verifies the checksum, installs to `~/.lagom`, and adds the PATH line, all idempotently. `sh install.sh --uninstall` reverses it.

Manual steps, if you prefer:

1. Unpack anywhere, e.g. `~/lagom` (the binary is built for Apple Silicon; on an Intel Mac, build from source with `cargo install --path crates/lagom_cli`).
2. If macOS refuses to open it ("cannot verify the developer" / "damaged"), run once:
   ```
   xattr -cr ~/lagom
   ```
   That's Gatekeeper reacting to the unsigned binary, not a real problem.
3. Add to PATH:
   ```
   echo 'export PATH="$PATH:'"$HOME"'/lagom"' >> ~/.zshrc
   source ~/.zshrc
   ```

## Install — Linux

**The easy way** (from a clone of the repo): `sh install.sh` — same deal: verify, install to `~/.lagom`, PATH line, idempotent; `sh install.sh --uninstall` reverses it.

Manual steps, if you prefer:

1. Unpack anywhere, e.g. `~/lagom`.
2. Add to PATH:
   ```
   echo 'export PATH="$PATH:'"$HOME"'/lagom"' >> ~/.bashrc   # or ~/.zshrc
   source ~/.bashrc
   ```

---

## Uninstall

An install is a folder plus (optionally) a PATH line — `lagom uninstall` removes both. It shows the plan first and deletes nothing until you say `--yes`.

Run it, read the plan, then run it again with `--yes`:

```
lagom uninstall --rc        # the plan: what will go, what is skipped and why
lagom uninstall --rc --yes  # do it
```

`--rc` also removes the PATH line the install steps above added (it rewrites only lines mentioning a `lagom` folder, and only in the shell rc files that have one). Open a **new** terminal afterwards.

It refuses to delete anything that is not a release install: a folder holding `Cargo.toml` (your source checkout) is skipped, as is a cargo `target` build cache — if you installed with `cargo install --path crates/lagom_cli`, remove it with `cargo uninstall lagom` instead.

---

## Verify the download (optional but easy)

```sh
sha256sum -c sha256sums.txt      # (run in the folder with the downloads)
```
Every line should say **OK**.

## First program

```
lagom new hello
cd hello
lagom run
```
(On Windows before PATH is set: `..\lagom.exe run`.)

## What works
- Output & input: `say "…"`, `ask "…"`
- Variables: `make x equal to 5`, mutable `make changing score equal to 0` + `set score to 10`
- Arithmetic, words and symbols: `6 times 7`, `20 divided by 4`, `remainder of 17 and 5`, `10 modulo 3`
- Branches: `if / otherwise if / otherwise`; loops: `repeat 10 times using i`, `repeat while …`, `repeat for each … in`, with `stop` / `next`
- Lists and maps: `a list of …`, `a map from … to …`
- Functions: `takes … called …` / `returns …` / `gives back …`
- Errors as values: `can fail`, `fail with`, `attempt … if it fails then … otherwise …`
- Structs, `test` blocks with `check that`, `use math for square root`
- Kinds and `match` with patterns; the full option/result model; closures with `map` / `keep` / `combine`; files and JSON (M1)
- **Tasks & channels (M2):** `start a task` / `wait for all tasks`, `a channel of number`, `send 5 to c`, `receive from c`, `keep going`
- **Generic types (M2):** `anything` type parameters, `some type that does comparable`
- **Causal diagnostics (M2):** every fix explains *why* it works; `lagom explain <code>` has a teaching page for every error code
- **REPL (M2):** `lagom play` — line-by-line with types shown, `#history` / `#replay`
- **Doc comments (M2):** `lagom doc` renders `##` comments to markdown; `lagom doc --check` runs `## >>>` examples against `## =` expected output
- **Packages (M2):** `lagom add <name> <path>` vendoring into `packages/`, recorded in `Lagom.toml`
- **Profiler (M2):** `lagom profile` — per-function call counts and the compile-time breakdown
- Commands: `lagom run | build | test | check | fmt | new | explain | words | play | doc | add | remove | profile | uninstall`

The full tour — every form with copy-paste commands and outputs — is [`demo.md`](https://github.com/Srihan-Yeleswarapu/lagom/blob/main/demo.md) in the repo.

## Troubleshooting

| Symptom | Fix |
|---|---|
| "the Lagom runtime library could not be built" | The `lib` folder is missing or was separated from the binary — keep them together. |
| Windows marks the download suspicious | File Properties → **Unblock** → OK (normal for unsigned tools). |
| macOS "cannot verify" / "damaged" | `xattr -cr <folder>` once (Gatekeeper). |
| `lagom` not found after adding PATH | Open a **new** terminal; PATH changes don't reach open ones. |

Found a bug? [Open an issue](https://github.com/Srihan-Yeleswarapu/lagom/issues).
