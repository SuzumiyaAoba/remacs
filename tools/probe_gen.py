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

def read_exprs(path):
    """Group file lines into complete exprs: a new expr starts at depth 0
    and continues until paren/bracket depth returns to 0.  Depth is
    tracked outside strings and `;' comments so multi-line forms wrap
    into a single record."""
    exprs, buf, depth = [], [], 0
    for line in open(path):
        s = line.strip()
        if depth == 0 and (not s or s.startswith(";")):
            continue
        buf.append(line.rstrip("\n"))
        instr, esc = False, False
        for c in line:
            if esc:
                esc = False
            elif c == "\\":
                esc = True
            elif instr:
                if c == '"':
                    instr = False
            elif c == '"':
                instr = True
            elif c == ";":
                break
            elif c in "([":
                depth += 1
            elif c in ")]":
                depth -= 1
        if depth <= 0:
            exprs.append("\n".join(buf))
            buf, depth = [], 0
    if buf:
        exprs.append("\n".join(buf))
    out = []
    for e in exprs:
        out.extend(unwrap_progn(e))
    return [e for e in out if e.strip() and not e.strip().startswith(";")]


def unwrap_progn(expr):
    """A top-level `(progn A B ...)' contributes its subforms as separate
    probe exprs, preserving per-call granularity; any other form stays
    whole."""
    body = expr.strip()
    if not body.startswith("(progn"):
        return [expr]
    inner = body[len("(progn"):]
    # Drop the final close paren of the progn itself.
    if inner.rstrip().endswith(")"):
        inner = inner.rstrip()[:-1]
    parts, buf, depth, instr, esc = [], "", 0, False, False
    atom = ""
    def flush_atom():
        nonlocal atom
        s = atom.strip()
        if s and not s.startswith(";"):
            parts.append(s)
        atom = ""
    def flush_buf():
        nonlocal buf
        s = buf.strip()
        if s and not s.startswith(";"):
            parts.append(s)
        buf = ""
    for line in inner.split("\n"):
        for c in line:
            if esc:
                if depth == 0:
                    atom += c
                else:
                    buf += c
                esc = False
            elif c == "\\":
                if depth == 0:
                    atom += c
                else:
                    buf += c
                esc = True
            elif instr:
                if depth == 0:
                    atom += c
                else:
                    buf += c
                if c == '"':
                    instr = False
            elif c == '"':
                if depth == 0:
                    atom += c
                else:
                    buf += c
                instr = True
            elif c == ";" and depth == 0:
                break
            elif c in "([":
                if depth == 0:
                    flush_atom()
                    buf = c
                else:
                    buf += c
                depth += 1
            elif c in ")]":
                depth -= 1
                buf += c
                if depth <= 0:
                    flush_buf()
                    depth = 0
            elif depth == 0:
                if c.isspace():
                    flush_atom()
                else:
                    atom += c
            else:
                buf += c
        if depth <= 0 and buf:
            flush_buf()
            depth = 0
    flush_atom()
    if buf:
        parts.append(buf)
    return [p for p in parts if p.strip() and not p.strip().startswith(";")]


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
                       capture_output=True, timeout=300)
    return records(outfile), p.stderr.decode("utf-8", errors="replace")

def main():
    verbose = "-v" in sys.argv
    exprfile, remacs = sys.argv[1], sys.argv[2]
    exprs = read_exprs(exprfile)

    gl, gerr = run([EMACS, "--batch", "-Q", "-l", "/tmp/probe.el"],
                   exprs, "/tmp/probe-recs-gnu.txt")
    rl, rerr = run([remacs, "-l", "/tmp/probe.el"],
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
