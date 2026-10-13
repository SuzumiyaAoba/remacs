#!/bin/sh
# Iterate: run GNU oracle; on abort/hang read last DIFF_PROGRESS line,
# add that name to tools/difftest.py SKIP-block list, regen, retry.
set -u
for i in $(seq 1 30); do
  rm -f /tmp/diff_gnu.prog
  DIFF_CALLS=tests/diff/calls.el DIFF_PROGRESS=/tmp/diff_gnu.prog \
    timeout 240 emacs -Q --batch -l tests/diff/runner.el \
    > /tmp/diff_gnu.out 2>/dev/null
  rc=$?
  lines=$(wc -l < /tmp/diff_gnu.out | tr -d ' ')
  last=$(tail -1 /tmp/diff_gnu.prog 2>/dev/null | awk '{print $2}')
  echo "round $i rc=$rc lines=$lines last=${last:-none}"
  if [ "$rc" -eq 0 ]; then echo "GNU complete"; break; fi
  if [ -z "$last" ]; then echo "no progress file"; break; fi
  # append offender to SKIP set literal in difftest.py
  python3 - "$last" <<'EOF'
import re,sys
name=sys.argv[1]
s=open('tools/difftest.py').read()
# add inside the `in ("...", ...)` tuple in extra_skip after the last
# quoted member before `)` followed by `:return True`
marker='"play-sound-file", "x-hide-tip",'
new='"play-sound-file", "x-hide-tip", "%s",' % name
assert marker in s
s=s.replace(marker,new,1)
open('tools/difftest.py','w').write(s)
EOF
  python3 tools/difftest.py gen >/dev/null 2>&1
done
echo done
