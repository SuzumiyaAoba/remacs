#!/usr/bin/env python3
"""Differential probe harness: run elisp exprs through GNU Emacs and remacs.

Each expr is wrapped so output is delimited by \x01 ... \x02 markers,
robust to embedded newlines in printed values.
Usage: probe_gen.py <exprfile> <remacs_bin> [-v]
"""
import subprocess, sys

EMACS = "/run/current-system/sw/bin/emacs"

def wrap(exprs):
    out = []
    for e in exprs:
        e = e.strip()
        if not e or e.startswith(";"):
            continue
        out.append(
            '(princ "\\x01")(princ (condition-case __e (prin1-to-string %s) '
            '(error (format "ERR:%%s" (car __e)))))(princ "\\x02")' % e
        )
    return "\n".join(out) + "\n"

def records(output):
    parts = output.split("\x01")
    recs = []
    for p in parts:
        if "\x02" in p:
            recs.append(p.split("\x02")[0])
    return recs

def main():
    verbose = "-v" in sys.argv
    exprfile, remacs = sys.argv[1], sys.argv[2]
    exprs = [l.strip() for l in open(exprfile).read().split("\n")
             if l.strip() and not l.startswith(";")]
    with open("/tmp/probe.el", "w") as f:
        f.write(wrap(exprs))

    g = subprocess.run([EMACS, "--batch", "-Q", "-l", "/tmp/probe.el"],
                       capture_output=True, text=True, timeout=120)
    r = subprocess.run([remacs, "--script", "/tmp/probe.el"],
                       capture_output=True, text=True, timeout=120)
    gl = records(g.stdout)
    rl = records(r.stdout)
    match, mism = 0, []
    for i, e in enumerate(exprs):
        gv = gl[i] if i < len(gl) else "<none>"
        rv = rl[i] if i < len(rl) else "<none>"
        if gv == rv:
            match += 1
        else:
            mism.append((e, gv, rv))
    print(f"MATCH {match}  MISMATCH {len(mism)}  of {len(exprs)}")
    show = mism if verbose else mism[:80]
    for e, gv, rv in show:
        print(f"  {e[:90]}\n    emacs:  {gv[:200]}\n    remacs: {rv[:200]}")
    if len(mism) > len(show):
        print(f"  ... and {len(mism)-len(show)} more")
    if g.stderr.strip():
        print("--- emacs stderr:", g.stderr.strip()[-200:])
    if r.stderr.strip():
        print("--- remacs stderr:", r.stderr.strip()[-200:])

if __name__ == "__main__":
    main()
