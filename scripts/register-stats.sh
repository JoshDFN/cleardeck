#!/usr/bin/env bash
#
# HOW MUCH IS LEFT, AS A COMMAND.
#
#   ./scripts/register-stats.sh          counts, and the open list
#   ./scripts/register-stats.sh --check  the same, plus the consistency gate (exit 1 on a problem)
#   ./scripts/register-stats.sh --open   just the open ids, one per line, for scripting
#
# WHY THIS EXISTS
#
# For eight waves the answer to "how much is open?" was an argument. docs/DEFECTS.md
# carried ELEVEN spellings of "fixed" -- `FIXED IN THIS PASS`, `FIXED-IN-WAVE-2`,
# `FIXED-IN-COHERENCE-PASS`, `fixed-in-wave-1`, `MOSTLY-FIXED-IN-WAVE-2`,
# `PARTLY-FIXED-IN-WAVE-2`, `CLOSED`, `answered`, `partly fixed`, `FIXED for X, OPEN
# for Y`, `open -- other owner` -- across 71 distinct status strings, so no tool and no
# human could count it, entries that were closed sat in the register looking open, and
# six of one auditor's ten findings turned out to have been filed already. The failure
# mode had stopped being "we cannot FIND things" and become "we cannot SCHEDULE them".
#
# So the status column is now a closed set of five words, the wave lives in its own
# column, and every FIXED row names the gate that would catch the defect coming back.
# This script is what makes that enforceable rather than aspirational.
#
# It reads the two canonical tables:
#   docs/DEFECTS.md           "## THE REGISTER"   rows: | [id](#anchor) | sev | status | wave | gate | where | one line |
#   docs/SECURITY-FINDINGS.md "## THE FINDINGS"   rows: | [FINDING nn](#finding-nn) | sev | status | wave | gate | DEFECTS.md | one line |
#
# --check enforces, and exits non-zero on any of:
#   1. a status outside the vocabulary
#   2. a FIXED row with no gate named
#   3. a FIXED-NO-GATE row that names one anyway (use FIXED)
#   4. an OPEN or WONTFIX row that claims a gate (BY-DESIGN may name the test that pins the
#      intended behaviour)
#   5. a table row whose anchor has no `<a id=...>` in the file
#   6. an entry heading in the file with no row in the table  (the way entries went missing)
#   7. a duplicate id
#   8. a DEFECTS.md id referenced by a findings row that does not exist

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

DEFECTS="docs/DEFECTS.md"
FINDINGS="docs/SECURITY-FINDINGS.md"

MODE="${1:-}"

if [ -t 1 ]; then B=$'\033[1m'; R=$'\033[0m'; G=$'\033[32m'; E=$'\033[31m'; Y=$'\033[33m'
else B=""; R=""; G=""; E=""; Y=""; fi

for f in "$DEFECTS" "$FINDINGS"; do
  [ -f "$f" ] || { printf '%sFATAL:%s %s is missing\n' "$E" "$R" "$f" >&2; exit 2; }
done

command -v python3 >/dev/null 2>&1 || { printf '%sFATAL:%s python3 is required\n' "$E" "$R" >&2; exit 2; }

python3 - "$MODE" "$DEFECTS" "$FINDINGS" <<'PY'
import re, sys, collections

mode, defects_path, findings_path = sys.argv[1], sys.argv[2], sys.argv[3]

VOCAB = ("OPEN", "FIXED", "FIXED-NO-GATE", "WONTFIX", "BY-DESIGN")
GATELESS = ("OPEN", "WONTFIX", "BY-DESIGN", "FIXED-NO-GATE")
SEVERITIES = ("fund-theft", "critical", "high", "medium", "low")

def cells(line):
    return [c.strip() for c in line.strip().strip('|').split('|')]

def anchors(text):
    return set(re.findall(r'<a\s+id="([^"]+)"', text))

def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()

def strip_md(s):
    s = re.sub(r'\*\*|\*|`', '', s)
    return s.strip()

problems = []

# ---------------------------------------------------------------- DEFECTS.md
dtext = read(defects_path)
dlines = dtext.split('\n')
danch = anchors(dtext)

# ONLY the canonical table counts. The per-wave queues further down the file are
# historical snapshots whose status columns are deliberately not maintained, and
# reading them was how the same id ended up with three different statuses.
def region(lines, anchor):
    try:
        start = next(i for i, l in enumerate(lines) if anchor in l)
    except StopIteration:
        return []
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if lines[i].startswith('## ') and i > start + 3:
            end = i
            break
    return [(i + 1, lines[i]) for i in range(start, end)]

ROW = re.compile(r'^\|\s*\[([ETHDL]-\d+)\]\(#([a-z0-9-]+)\)\s*\|')
drows = []
for n, l in region(dlines, '<a id="the-register"></a>'):
    m = ROW.match(l)
    if not m:
        continue
    c = cells(l)
    if len(c) < 5:
        problems.append(f"{defects_path}:{n}: register row has {len(c)} columns, needs at least 5")
        continue
    drows.append({
        "line": n, "id": m.group(1), "anchor": m.group(2),
        "sev": strip_md(c[1]), "status": strip_md(c[2]),
        "wave": strip_md(c[3]), "gate": c[4].strip(),
    })

if not drows:
    problems.append(f"{defects_path}: no register rows found at all -- the table moved or its shape changed")

seen = collections.Counter(r["id"] for r in drows)
for i, k in seen.items():
    if k > 1:
        problems.append(f"{defects_path}: id {i} appears in {k} register rows; an id must name exactly one defect")

for r in drows:
    if r["status"] not in VOCAB:
        problems.append(f"{defects_path}:{r['line']}: {r['id']} status {r['status']!r} is not one of {'/'.join(VOCAB)}")
    if r["sev"] not in SEVERITIES:
        problems.append(f"{defects_path}:{r['line']}: {r['id']} severity {r['sev']!r} is not one of {'/'.join(SEVERITIES)}")
    gate_named = r["gate"] not in ("", "—", "-", "none") and not r["gate"].startswith("—")
    if r["status"] == "FIXED" and not gate_named:
        problems.append(f"{defects_path}:{r['line']}: {r['id']} is FIXED and names no gate. "
                        f"A fix with no gate is a memory: use FIXED-NO-GATE and say so.")
    if r["status"] == "FIXED-NO-GATE" and r["gate"] != "none":
        problems.append(f"{defects_path}:{r['line']}: {r['id']} is FIXED-NO-GATE, so its gate cell must be exactly `none`")
    if r["status"] in ("OPEN", "WONTFIX") and gate_named:
        problems.append(f"{defects_path}:{r['line']}: {r['id']} is {r['status']} but names a gate; a gate on an open defect is a gate that is red or blind")
    if r["anchor"] not in danch:
        problems.append(f"{defects_path}:{r['line']}: {r['id']} links to #{r['anchor']}, which no <a id=…> in this file provides")

# every entry heading must have a row
HEAD = re.compile(r'^#{2,4}\s+\**\[?([ETHDL]-\d+)\b')
have = {r["id"] for r in drows}
heading_ids = []
for n, l in enumerate(dlines, 1):
    m = HEAD.match(l)
    if m and m.group(1) not in heading_ids:
        heading_ids.append(m.group(1))
for i in heading_ids:
    if i not in have:
        problems.append(f"{defects_path}: {i} has a write-up but NO row in THE REGISTER, so nothing counts it")

# ------------------------------------------------------- SECURITY-FINDINGS.md
ftext = read(findings_path)
flines = ftext.split('\n')
fanch = anchors(ftext)

FROW = re.compile(r'^\|\s*\[FINDING (\d+)\]\(#(finding-\d+)\)\s*\|')
frows = []
for n, l in region(flines, '<a id="the-findings"></a>'):
    m = FROW.match(l)
    if not m:
        continue
    c = cells(l)
    if len(c) < 6:
        problems.append(f"{findings_path}:{n}: findings row has {len(c)} columns, needs at least 6")
        continue
    frows.append({
        "line": n, "id": "FINDING " + m.group(1), "anchor": m.group(2),
        "sev": strip_md(c[1]), "status": strip_md(c[2]),
        "wave": strip_md(c[3]), "gate": c[4].strip(), "defect": c[5].strip(),
    })

if not frows:
    problems.append(f"{findings_path}: no findings rows found at all -- the table moved or its shape changed")

fseen = collections.Counter(r["id"] for r in frows)
for i, k in fseen.items():
    if k > 1:
        problems.append(f"{findings_path}: {i} appears in {k} rows; two agents numbering findings independently is how 21/22/23 collided")

for r in frows:
    if r["status"] not in VOCAB:
        problems.append(f"{findings_path}:{r['line']}: {r['id']} status {r['status']!r} is not one of {'/'.join(VOCAB)}")
    if r["sev"] not in SEVERITIES:
        problems.append(f"{findings_path}:{r['line']}: {r['id']} severity {r['sev']!r} is not one of {'/'.join(SEVERITIES)}")
    gate_named = r["gate"] not in ("", "—", "-", "none") and not r["gate"].startswith("—")
    if r["status"] == "FIXED" and not gate_named:
        problems.append(f"{findings_path}:{r['line']}: {r['id']} is FIXED and names no gate")
    if r["status"] == "FIXED-NO-GATE" and r["gate"] != "none":
        problems.append(f"{findings_path}:{r['line']}: {r['id']} is FIXED-NO-GATE, so its gate cell must be exactly `none`")
    if r["anchor"] not in fanch:
        problems.append(f"{findings_path}:{r['line']}: {r['id']} links to #{r['anchor']}, which no <a id=…> in this file provides")
    for ref in re.findall(r'\[([ETHDL]-\d+)\]', r["defect"]):
        if ref not in have:
            problems.append(f"{findings_path}:{r['line']}: {r['id']} points at {ref}, which is in no DEFECTS.md register row")

FHEAD = re.compile(r'^#{2,3}\s+\**\[?FINDING (\d+)')
fhave = {r["id"] for r in frows}
for n, l in enumerate(flines, 1):
    m = FHEAD.match(l)
    if m and ("FINDING " + m.group(1)) not in fhave:
        problems.append(f"{findings_path}:{n}: FINDING {m.group(1)} has a write-up but NO row in THE FINDINGS")

# ------------------------------------------------------------------ reporting
def table(rows, title, width=None):
    by_status = collections.Counter(r["status"] for r in rows)
    print(f"\n\033[1m{title}\033[0m  ({len(rows)} entries)")
    for s in VOCAB:
        if by_status.get(s):
            print(f"    {by_status[s]:4d}  {s}")
    unknown = {s: n for s, n in by_status.items() if s not in VOCAB}
    for s, n in unknown.items():
        print(f"    {n:4d}  \033[31m{s}  <-- not in the vocabulary\033[0m")
    print(f"    ----")
    print(f"    {len(rows):4d}  total")
    op = [r for r in rows if r["status"] == "OPEN"]
    if op:
        print(f"\n    OPEN by severity:")
        sev = collections.Counter(r["sev"] for r in op)
        for s in SEVERITIES:
            if sev.get(s):
                print(f"      {sev[s]:4d}  {s}")

if mode == "--open":
    for r in drows:
        if r["status"] == "OPEN":
            print(f"{r['id']}\t{r['sev']}")
    for r in frows:
        if r["status"] == "OPEN":
            # no space in the id: --open is meant to be cut/awk-able
            print(f"{r['id'].replace(' ', '-')}\t{r['sev']}")
    sys.exit(0)

table(drows, "docs/DEFECTS.md — THE REGISTER")
table(frows, "docs/SECURITY-FINDINGS.md — THE FINDINGS")

nogate = [r for r in drows + frows if r["status"] == "FIXED-NO-GATE"]
if nogate:
    print(f"\n\033[1mFIXED WITH NO GATE\033[0m  ({len(nogate)}) — a fix nothing would catch coming back")
    for r in nogate:
        print(f"    {r['id']:<12s} {r['sev']}")

ungated_gate = [r for r in drows + frows
                if r["status"] == "FIXED" and "NOT RUN" in r["gate"]]
if ungated_gate:
    print(f"\n\033[1mGATE EXISTS BUT NO TARGET RUNS IT\033[0m  ({len(ungated_gate)}) — see H-45/H-46")
    for r in ungated_gate:
        print(f"    {r['id']:<12s} {r['sev']}")

print("\n\033[1mOPEN, worst first\033[0m")
rank = {s: i for i, s in enumerate(SEVERITIES)}
for r in sorted([x for x in drows + frows if x["status"] == "OPEN"],
                key=lambda x: (rank.get(x["sev"], 9), x["id"])):
    print(f"    {r['sev']:<10s} {r['id']}")

if mode == "--check":
    print()
    if problems:
        print(f"\033[31mREGISTER IS INCONSISTENT: {len(problems)} problem(s)\033[0m")
        for p in problems:
            print(f"    {p}")
        sys.exit(1)
    print("\033[32mregister is consistent: vocabulary closed, every FIXED names a gate, every anchor resolves, every entry counted\033[0m")
elif problems:
    print(f"\n\033[33m{len(problems)} consistency problem(s); run with --check to list them\033[0m")

sys.exit(0)
PY
