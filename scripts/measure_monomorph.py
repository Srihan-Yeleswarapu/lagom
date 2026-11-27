#!/usr/bin/env python3
"""Measure the monomorphization-policy datum (doc 08 §5, M2 decision).

The compiler implements the *shared* arm of the policy today: one compiled
body per generic function (`takes anything ...`), executed over the runtime's
tagged value representation — nothing is cloned per instantiation yet. This
script measures what that means in practice, on the **native backend** (the
backend the user runs; the interpreter dispatches through the same tagged
values either way):

A. Per-call flow-through cost — a hot loop calling a generic identity
   (`takes anything`) versus a hand-specialized one (`takes number`).
   Release builds; wall clock, best of several runs.

B. Instantiation churn — N generic functions (each called once) versus the
   same program shape with N typed functions: compile wall time and binary
   size as N grows. The shared arm predicts flat growth; specialization
   would grow linearly with per-instantiation clones.

Arithmetic inside a generic body is not attempted: at M2 sema refuses
operators on `anything` (`plus` needs a numeric type), so the reachable
generic surface is pass-through, combinators, and builtins that already
operate on tagged values. The datum records the cost of what exists.

Usage: python3 scripts/measure_monomorph.py
Writes nothing outside the system temp dir. Requires a built CLI
(`cargo build --release -p lagom_cli`); never rebuilds silently.
"""

import argparse, statistics, subprocess, sys, tempfile, time, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = os.path.join(ROOT, "target", "release", "lagom.exe" if os.name == "nt" else "lagom")


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def verify_binary():
    if not os.path.exists(CLI):
        print(f"FATAL: no release binary at {CLI} — run `cargo build --release -p lagom_cli` first")
        sys.exit(2)
    r = sh([CLI, "version"])
    if r.returncode != 0:
        print("FATAL: the release binary does not run")
        print(r.stderr)
        sys.exit(2)


def best_of(n, fn):
    runs = [fn() for _ in range(n)]
    return min(runs)


def timed_build(src, tmp, stem):
    out = os.path.join(tmp, f"{stem}_built")
    if os.path.exists(out):
        os.remove(out)
    start = time.perf_counter()
    r = sh([CLI, "build", src, "-o", out, "--release"], cwd=tmp)
    wall = time.perf_counter() - start
    if r.returncode != 0:
        print(f"FATAL: {stem} did not compile:")
        print(r.stdout[-4000:] + r.stderr[-4000:])
        sys.exit(2)
    return out, wall, os.path.getsize(out)


def timed_run(exe, tmp):
    start = time.perf_counter()
    r = subprocess.run([exe], capture_output=True, cwd=tmp)
    wall = time.perf_counter() - start
    if r.returncode != 0:
        print(r.stdout.decode()[-2000:], (r.stderr or b"")[-2000:])
        print("FATAL: a generated benchmark program failed at run time")
        sys.exit(2)
    return wall


def src_pass(shape, loop, k=1, extra_depth=1):
    """A hot loop calling k-argument identity/sum functions round-robin.

    shape "generic": every function `takes anything` (the shared body).
    shape "typed":   every function `takes number` (specialization's result:
                     the same program, hand-monomorphized).
    extra_depth: how many calls deep a loop iteration goes (call-chain cost).
    """
    t = "anything" if shape == "generic" else "number"
    lines = []
    for i in range(1, k + 1):
        lines.append(f"function digger{i}\n    takes {t} called x\n    gives back x")
    lines.append("")
    lines.append("make changing total equal to 0")
    lines.append(f"repeat {loop} times using i")
    for _ in range(extra_depth):
        lines.append("    set total to digger1 total")
    lines.append("")
    lines.append('say "done"')
    return "\n".join(lines) + "\n"


def src_churn(shape, n):
    """N tiny functions, one call each (both shapes: only the param type
    differs) — the compile-time/binary-size dimension."""
    t = "anything" if shape == "generic" else "number"
    lines = []
    for i in range(1, n + 1):
        lines.append(f"function pick{i}\n    takes {t} called x\n    gives back x")
    lines.append("")
    lines += [f"say pick{i} {i}" for i in range(1, n + 1)]
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5, help="timed repetitions per shape")
    ap.add_argument("--loop", type=int, default=2000000, help="hot-loop iteration count (A)")
    ap.add_argument("--depth", type=int, default=2, help="calls per loop iteration (A)")
    args = ap.parse_args()
    verify_binary()

    tmp = tempfile.mkdtemp(prefix="lagom_mono_")
    print(f"binary: {CLI}")
    print(f"A: hot flow-through loop, iterations={args.loop}, depth={args.depth}, best of {args.runs}")
    print("   shape      wall s / call ns   (release native)")
    for shape in ("generic", "typed"):
        src = os.path.join(tmp, f"a_{shape}.lagom")
        open(src, "w").write(src_pass(shape, args.loop, 1, args.depth))
        exe, _, _ = timed_build(src, tmp, f"a_{shape}")
        wall = best_of(args.runs, lambda: timed_run(exe, tmp))
        per_call = wall / (args.loop * args.depth) * 1e9
        print(f"   {shape:<10} {wall:8.3f}   {per_call:8.1f} ns/call")

    # The churn sweep stops at 240 deliberately. On the M2 dev machine,
    # linking a Lagom object with >= 254 top-level functions crashes Apple's
    # ld (Trace/BPT trap; reproduced in dev and release; Apple clang 21,
    # darwin27.2), while the Lagom objects themselves grow linearly and
    # cleanly through the boundary (verified at N = 250..258 by object size
    # and symbol counts). See doc 08 §"The M2 decision" for the record.
    print(f"\nB: instantiation churn, best of {args.runs} builds")
    print("   N    shape     compile s   binary KiB")
    for n in (16, 64, 240):
        for shape in ("generic", "typed"):
            src = os.path.join(tmp, f"b_{shape}_{n}.lagom")
            open(src, "w").write(src_churn(shape, n))
            exe, wall, size = best_of(args.runs, lambda: timed_build(src, tmp, f"b_{shape}_{n}"))
            print(f"   {n:<4} {shape:<9} {wall:9.3f}   {size//1024:9d}")

    print("\n(note: B's clone-per-instantiation arm would add one body per N;")
    print(" today the shared arm compiles each generic function once — the")
    print(" growth you see is the frontend work for N declarations, identical")
    print(" in both shapes by construction.)")


if __name__ == "__main__":
    main()
