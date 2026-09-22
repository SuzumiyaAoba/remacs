#!/usr/bin/env python3
"""Differential probe harness: run elisp exprs through GNU Emacs and remacs.

Each expr's prin1 output is base64-encoded and appended to a per-side
records file via write-region, so printed values containing control
bytes or non-ASCII text cannot desync parsing, and a process abort
only loses the record being printed.
Usage: probe_gen.py <exprfile> <remacs_bin> [-v]
"""
import base64, os, subprocess, sys

EMACS = "/run/current-system/sw/bin/emacs"

def wrap(exprs, outfile):
    out = ['(setq __outfile %s)' % elisp_str(outfile)]
    for e in exprs:
        e = e.strip()
        if not e or e.startswith(";"):
            continue
        out.append(
            '(write-region (concat (base64-encode-string '
            '(encode-coding-string (condition-case __e '
            '(prin1-to-string %s) '
            '(error (format "ERR:%%s" (car __e)))) (quote utf-8) t) t) "\\n") '
            'nil __outfile t (quote quiet))' % e
        )
    return "\n".join(out) + "\n"

def elisp_str(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'

def records(path):
    recs = []
    if os.path.exists(path):
        for line in open(path, "rb").read().split(b"\n"):
            line = line.strip()
            if not line:
                continue
            try:
                pad = b"=" * (-len(line) % 4)
                recs.append(base64.b64decode(line + pad)
                            .decode("utf-8", errors="replace"))
            except Exception:
                recs.append("<bad-b64:" + line[:60].decode("ascii",
                                                           "replace") + ">")
    return recs

def run(prog_argv, exprs, outfile):
    if os.path.exists(outfile):
        os.unlink(outfile)
    with open("/tmp/probe.el", "w") as f:
        f.write(wrap(exprs, outfile))
    p = subprocess.run(prog_argv[:-1] + ["/tmp/probe.el"],
                       capture_output=True, text=True, timeout=300)
    return records(outfile), p.stderr

def main():
    verbose = "-v" in sys.argv
    exprfile, remacs = sys.argv[1], sys.argv[2]
    exprs = [l.strip() for l in open(exprfile).read().split("\n")
             if l.strip() and not l.startswith(";")]

    gl, gerr = run([EMACS, "--batch", "-Q", "-l", "/tmp/probe.el"],
                   exprs, "/tmp/probe-recs-gnu.txt")
    rl, rerr = run([remacs, "--script", "/tmp/probe.el"],
                   exprs, "/tmp/probe-recs-remacs.txt")
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
    if gerr.strip():
        print("--- emacs stderr:", gerr.strip()[-200:])
    if rerr.strip():
        print("--- remacs stderr:", rerr.strip()[-200:])

if __name__ == "__main__":
    main()
