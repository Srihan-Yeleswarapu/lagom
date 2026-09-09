"""Spot-check demo.md: run every M0-labeled section's commands and compare
output with the documented transcript."""
import re, subprocess, os, sys, tempfile

MD = r"C:\Users\Madhu_Chintapalli\Srihan\Documents\playground\own-coding-language\demo.md"
L = r"C:\Users\Madhu_Chintapalli\Srihan\Documents\playground\own-coding-language\target\release\lagom.exe"

text = open(MD, encoding="utf8").read()
# Split only on REAL section headings: `## <number>.` — bare `##` Lagom
# doc-comments inside code fences also start at column 0.
sections = re.split(r"\n(?=## \d+\.)", text)[1:]

def extract(section):
    """Return list of (lagom_source, expected_output) pairs."""
    pairs = []
    # split into commands
    cmds = re.split(r"### Command \d+", section)[1:]
    for c in cmds:
        # The first ```lagom block before 'Terminal transcript' is the
        # runnable command; blocks after it are setup fragments.
        head = c.split("Terminal transcript:")[0]
        m_src = re.search(r"```lagom\n(.*?)```", head, re.S)
        # transcript: first sh block after "Terminal transcript"
        m_tr = re.search(r"Terminal transcript:.*?```sh\n(.*?)```", c, re.S)
        if not m_src:
            continue
        src = m_src.group(1)
        expected = None
        if m_tr:
            lines = [l for l in m_tr.group(1).strip().splitlines() if l.strip()]
            # drop "$ lagom ..." invocation lines and blank
            lines = [l for l in lines if not l.strip().startswith("$")]
            expected = "\n".join(lines) if lines else ""
        pairs.append((src, expected))
    return pairs

ok = fail = err = 0
failures = []
tmp = tempfile.mkdtemp()
for sec in sections:
    title = sec.splitlines()[0]
    if "M1+" in title or "M2+" in title or "M3+" in title or "M4+" in title or "Expert" in title:
        continue
    for i, (src, expected) in enumerate(extract(sec), 1):
        path = os.path.join(tmp, f"s{i}.lagom")
        open(path, "w", encoding="utf8").write(src)
        r = subprocess.run([L, "run", path], capture_output=True, timeout=60,
                           input=b"", cwd=tmp)
        out = (r.stdout.decode("utf8", "replace") + r.stderr.decode("utf8", "replace")).strip()
        if r.returncode != 0:
            err += 1
            failures.append(f"[ERR exit={r.returncode}] {title} cmd{i}\n  src: {src.splitlines()[0][:70]}\n  got: {out.splitlines()[0][:90] if out else '(no output)'}")
        elif expected is not None and out != expected.strip():
            fail += 1
            w = expected.strip().splitlines()[0][:80] if expected.strip() else '(empty)'
            g = out.splitlines()[0][:80] if out else '(no output)'
            failures.append(f"[MISMATCH] {title} cmd{i}\n  src: {src.splitlines()[0][:70]}\n  want: {w}\n  got:  {g}")
        else:
            ok += 1

print(f"M0 commands: {ok+fail+err}  match: {ok}  mismatch: {fail}  error: {err}")
print()
for f in failures[:40]:
    print(f)
