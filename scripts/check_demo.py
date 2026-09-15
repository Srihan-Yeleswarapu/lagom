"""Check demo.md against the real compiler: run every documented command and
compare output with the documented transcript — the gate behind "docs and
demos stay synchronized with real compiler behavior".

Conventions demo.md may use (all machine-checked here):
- ``$ lagom run file.lagom``            -> runs with `lagom run`, no input
- ``$ lagom run file.lagom < input``    -> the named stdin file feeds `ask`
- ``$ lagom test file.lagom``           -> runs with `lagom test`
- a transcript line of exactly ``<varies>`` marks nondeterministic output:
  the command must merely exit 0 and print something.
Every command must be self-contained: demo commands never depend on earlier
commands (each section's setup is written into the same block).

Freshness: the script rebuilds the release binary first (`cargo build
--release -p lagom_cli`), so a stale binary can never certify transcripts —
cargo's own fingerprinting makes the no-op rebuild cheap. A failed build is
fatal (exit 2), never a silent pass.

Milestones: a section's label (after the em-dash, last `/` token) says which
milestone the syntax belongs to; sections labeled M0/M1+/M2+ (or unlabeled,
which means M0) are implemented and MUST execute — there are no silent
skips. Sections whose title carries `` (planned)`` document syntax the
compiler does not implement yet (the marker is part of the catalog, see the
status legend in demo.md); they are skipped loudly, counted, and listed.
Sections labeled for future milestones (M3+ and later) are skipped the same
way. When a transcript mismatches real behavior, that is a finding: fix the
transcript, or the code if the transcript matches the architecture and the
code drifted.
"""
import re, subprocess, os, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MD = os.path.join(ROOT, "demo.md")
L = os.path.join(ROOT, "target", "release",
                 "lagom.exe" if os.name == "nt" else "lagom")

# Milestones whose language the compiler implements (doc 10's roadmap:
# M0, M1 done; M2 done). A section labeled for a later milestone is future
# syntax: skipped, but loudly (counted and listed, never silent).
IMPLEMENTED_LABELS = {"M0", "M1+", "M2+"}

def ensure_fresh_binary():
    """Rebuild the release binary; a stale binary must never certify docs."""
    r = subprocess.run(
        ["cargo", "build", "--release", "-p", "lagom_cli"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if r.returncode != 0 or not os.path.exists(L):
        print("FATAL: the release binary could not be brought current —")
        print("`cargo build --release -p lagom_cli` failed; refusing to")
        print("certify transcripts against a stale or missing binary.\n")
        print(r.stdout[-2000:])
        print(r.stderr[-2000:])
        sys.exit(2)

def section_label(title):
    """The milestone label of a section title, or None when unlabeled."""
    if "—" not in title:
        return None
    tail = title.split("—")[-1]
    miles = [t.strip() for t in tail.split("/")
             if re.fullmatch(r"M\d\+", t.strip()) or t.strip() == "M0"]
    return miles[-1] if miles else None

def is_implemented(title):
    # `(planned)` is the catalog's own marker (status legend in demo.md) for
    # documented syntax the compiler does not implement yet.
    if "(planned)" in title:
        return False
    lab = section_label(title)
    return lab is None or lab in IMPLEMENTED_LABELS

text = open(MD, encoding="utf8").read()
# Split only on REAL section headings: `## <number>.` — bare `##` Lagom
# doc-comments inside code fences also start at column 0.
sections = re.split(r"\n(?=## \d+\.)", text)[1:]

def extract(section):
    """Return list of (lagom_source, expected_output, cmd, stdin) tuples."""
    pairs = []
    cmds = re.split(r"### Command \d+", section)[1:]
    for c in cmds:
        head = c.split("Terminal transcript:")[0]
        m_src = re.search(r"```lagom\n(.*?)```", head, re.S)
        m_tr = re.search(r"Terminal transcript:.*?```sh\n(.*?)```", c, re.S)
        if not m_src:
            continue
        src = m_src.group(1)
        cmd = "run"
        stdin = b""
        expected = None
        if m_tr:
            tr = m_tr.group(1)
            m_inv = re.search(r"^\$\s+(.*)$", tr, re.M)
            if m_inv:
                invocation = m_inv.group(1).strip()
                if "test" in invocation.split()[1:2]:
                    cmd = "test"
                m_in = re.search(r"<\s*(\S+\.txt)", invocation)
                if m_in:
                    # stdin file: content given in a ``` block named after it
                    fname = m_in.group(1)
                    m_file = re.search(r"```" + re.escape(fname) + r"\n(.*?)```", c, re.S)
                    stdin = (m_file.group(1) if m_file else "").encode("utf8")
            lines = [l for l in tr.strip().splitlines() if l.strip()]
            lines = [l for l in lines if not l.strip().startswith("$")]
            expected = "\n".join(lines) if lines else ""
        pairs.append((src, expected, cmd, stdin))
    return pairs

ensure_fresh_binary()

ok = fail = err = 0
failures = []
exec_secs = 0
exec_labels = set()
skip_secs = 0
skip_cmds = 0
skip_labels = set()
skip_numbers = []
planned_secs = 0
planned_cmds = 0
planned_numbers = []
tmp = tempfile.mkdtemp()
for sec in sections:
    title = sec.splitlines()[0]
    if not is_implemented(title):
        m_num = re.match(r"## (\d+)\.", title)
        if "(planned)" in title:
            planned_secs += 1
            planned_cmds += len(extract(sec))
            if m_num:
                planned_numbers.append(m_num.group(1))
        else:
            skip_secs += 1
            skip_labels.add(section_label(title) or "(unlabeled)")
            skip_cmds += len(extract(sec))
            if m_num:
                skip_numbers.append(m_num.group(1))
        continue
    pairs = extract(sec)
    if not pairs:
        continue
    exec_secs += 1
    lab = section_label(title)
    exec_labels.add(lab if lab is not None else "M0 (unlabeled)")
    for i, (src, expected, cmd, stdin) in enumerate(pairs, 1):
        path = os.path.join(tmp, f"s{i}.lagom")
        open(path, "w", encoding="utf8").write(src)
        r = subprocess.run([L, cmd, path], capture_output=True, timeout=60,
                           input=stdin, cwd=tmp)
        out = (r.stdout.decode("utf8", "replace") + r.stderr.decode("utf8", "replace")).replace("\r\n", "\n").strip()
        out = "\n".join([x for x in out.splitlines() if x.strip()])

        varies = expected is not None and "<varies>" in expected
        if r.returncode != 0 and not (varies and out):
            err += 1
            failures.append(f"[ERR exit={r.returncode}] {title} cmd{i}\n  src: {src.splitlines()[0][:70]}\n  got: {out.splitlines()[0][:90] if out else '(no output)'}")
        elif varies:
            ok += 1 if out else 0
            if not out:
                err += 1
                failures.append(f"[ERR no output] {title} cmd{i}\n  src: {src.splitlines()[0][:70]}")
        elif expected is not None and out != expected.strip():
            fail += 1
            w = expected.strip().splitlines()[0][:80] if expected.strip() else '(empty)'
            g = out.splitlines()[0][:80] if out else '(no output)'
            failures.append(f"[MISMATCH] {title} cmd{i}\n  src: {src.splitlines()[0][:70]}\n  want: {w}\n  got:  {g}")
        else:
            ok += 1

total = ok + fail + err
print(f"executed: {total} commands in {exec_secs} sections "
      f"(labels: {', '.join(sorted(exec_labels))})")
print(f"  match: {ok}  mismatch: {fail}  error: {err}")
if skip_secs:
    print(f"skipped (future milestones, not silent): {skip_cmds} commands "
          f"in {skip_secs} sections (labels: {', '.join(sorted(skip_labels))})")
    print(f"  sections: {', '.join(skip_numbers)}")
else:
    print("skipped: none — every future-milestone section executed")
if planned_secs:
    print(f"skipped (marked `(planned)` in demo.md — documented syntax not "
          f"implemented yet): {planned_cmds} commands in {planned_secs} sections")
    print(f"  sections: {', '.join(planned_numbers)}")
print()
for f in failures:
    print(f)
sys.exit(1 if (fail or err) else 0)
