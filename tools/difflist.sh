#!/bin/sh
# difflist.sh NAME[,NAME...] — run driver rows for the names on both
# sides and print divergent rows only.
set -u
NAMES="${1:?usage: difflist.sh name[,name...]}"
DIFF_DIR="/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/diff"
REMACS="/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/target/release/remacs"
cd "$DIFF_DIR/sandbox" || exit 1
export DIFF_DEBUG=1 LC_ALL=C TZ=UTC DIFF_DIR
export DIFF_ONLY="$NAMES"
emacs -Q --batch -l ../driver.el 2>&1 | grep "^DIF|" > /tmp/dl.gnu 2>&1
"$REMACS" --batch -l ../driver.el 2>&1 | grep "^DIF|" > /tmp/dl.rem 2>&1
diff /tmp/dl.gnu /tmp/dl.rem | head -80
