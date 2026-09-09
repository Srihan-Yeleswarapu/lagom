# Lagom __VERSION__ — the M0 compiler

One binary plus a bundled runtime. **No Rust toolchain, no source checkout** — download the file for your OS, unpack, run.

| File | For |
|---|---|
| `lagom-Windows.zip` | Windows 10/11 (x64) |
| `lagom-Linux.tar.gz` | Linux (x64) |
| `lagom-macOS.tar.gz` | macOS |

`sha256sums.txt` lets you verify what you downloaded (see below).

**The one rule:** keep `lagom` (or `lagom.exe`) and its `lib` folder together, wherever you unpack — the binary finds its bundled runtime next to itself.

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

1. Unpack anywhere, e.g. `~/lagom`.
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

1. Unpack anywhere, e.g. `~/lagom`.
2. Add to PATH:
   ```
   echo 'export PATH="$PATH:'"$HOME"'/lagom"' >> ~/.bashrc   # or ~/.zshrc
   source ~/.bashrc
   ```

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

## What works in M0

- Output & input: `say "…"`, `ask "…"`
- Variables: `make x equal to 5`, mutable `make changing score equal to 0` + `set score to 10`
- Arithmetic, words and symbols: `6 times 7`, `20 divided by 4`, `remainder of 17 and 5`, `10 modulo 3`
- Branches: `if / otherwise if / otherwise`; loops: `repeat 10 times using i`, `repeat while …`, `repeat for each … in`, with `stop` / `next`
- Lists and maps: `a list of …`, `a map from … to …`
- Functions: `takes … called …` / `returns …` / `give back …`
- Errors as values: `can fail`, `fail with`, `attempt … if it fails then … otherwise …`
- Structs, `test` blocks with `check that`, `use math for square root`
- Commands: `lagom run | build | test | check | fmt | new | explain | words`

The full tour — every form with copy-paste commands and outputs — is [`demo.md`](https://github.com/Srihan-Yeleswarapu/lagom/blob/main/demo.md) in the repo.

## Troubleshooting

| Symptom | Fix |
|---|---|
| "the Lagom runtime library could not be built" | The `lib` folder is missing or was separated from the binary — keep them together. |
| Windows marks the download suspicious | File Properties → **Unblock** → OK (normal for unsigned tools). |
| macOS "cannot verify" / "damaged" | `xattr -cr <folder>` once (Gatekeeper). |
| `lagom` not found after adding PATH | Open a **new** terminal; PATH changes don't reach open ones. |

Found a bug? [Open an issue](https://github.com/Srihan-Yeleswarapu/lagom/issues).
