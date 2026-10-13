#!/bin/sh
# Continuous differential loop: run a full sweep, triage new divergences
# into tests/diff/known.txt when state-verified, repeat.  Logs land in
# tests/diff/loop-<n>.log; the latest report is tests/diff/report.txt.
# Stop by removing tests/diff/.loop-run.
set -u
D=tests/diff
touch "$D/.loop-run"
n=0
while [ -f "$D/.loop-run" ]; do
  n=$((n+1))
  python3 tools/difftest.py --no-regen --timeout 30 > "$D/loop-$n.log" 2>&1
  # print only the un-pinned divergence names for review
  python3 - "$D" <<'PY'
import sys, re
d = sys.argv[1]
div = {l.split()[1] for l in open(f"{d}/report.txt") if l.startswith('DIVERG')}
kn = {l.split('|')[0].strip() for l in open(f"{d}/known.txt")
      if l.strip() and not l.startswith('#')}
new = sorted(div - kn)
print(f"cycle: {len(div)} divergences, {len(new)} unpinned: {new}")
PY
done
echo "loop stopped after $n cycles"
