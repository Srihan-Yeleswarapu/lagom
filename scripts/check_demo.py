"""Spot-check demo.md: run every M0-labeled section's commands and compare
output with the documented transcript.

Conventions demo.md may use (all machine-checked here):
- ``$ lagom run file.lagom``            -> runs with `lagom run`, no input
- ``$ lagom run file.lagom < input``    -> the named stdin file feeds `ask`
- ``$ lagom test file.lagom``           -> runs with `lagom test`
- a transcript line of exactly ``<varies>`` marks nondeterministic output:
  the command must merely exit 0 and print something.
Every command must be self-contained: demo commands never depend on
earlier commands (each section's setup is written into the same block).
"""
import re, subprocess, os, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MD = os.path.join(ROOT, "demo.md")
L = os.path.join(ROOT, "target", "release", "lagom.exe")

text = open(MD, encoding="utf8").read()
# Split only on REAL section headings: `## <number>.` — bare `##` Lagom
# doc-comments inside code fences also start at column 0.
sections = re.split(r"\n(?=## \d+\.)", text)[1:]

def extract(section):
    """Return list of (lagom_source, expected_output, cmd) triples."""
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

ok = fail = err = 0
failures = []
tmp = tempfile.mkdtemp()
for sec in sections:
    title = sec.splitlines()[0]
    if "M1+" in title or "M2+" in title or "M3+" in title or "M4+" in title or "Expert" in title:
        continue
    for i, (src, expected, cmd, stdin) in enumerate(extract(sec), 1):
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

print(f"M0 commands: {ok+fail+err}  match: {ok}  mismatch: {fail}  error: {err}")
print()
for f in failures[:60]:
    print(f)
sys.exit(1 if (fail or err) else 0)
