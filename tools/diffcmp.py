#!/usr/bin/env python3
"""diffcmp.py GNU.out remacs.out → divergence report.

Rows: "IDX|TAG|PAYLOAD|PRINTED". TAG is VAL or ERR.
- Missing row on remacs side => CRASH (process died).
- Extra row on remacs side => EXTRA (GNU crashed/skipped).
- Different TAG or payload => DIV.
Payloads are compared literally; payloads containing "#<" (address
-bearing printouts) are compared with addresses normalized.
"""
import re
import sys

ADDR = re.compile(r"#<([^ ]*)")


def norm(payload: str) -> str:
    return ADDR.sub("#<\\1", payload)


def rows(path):
    d = {}
    order = []
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        m = re.match(r"^(\d+)\|([A-Z]+)\|(.*)$", line)
        if not m:
            continue
        idx = int(m.group(1))
        d[idx] = line[m.end(1) :]
        order.append(idx)
    return d, order


def main():
    g, gorder = rows(sys.argv[1])
    r, rorder = rows(sys.argv[2])
    divs = []
    missing = []
    extra = []
    for i in gorder:
        if i not in r:
            missing.append(i)
            continue
        gl, rl = g[i], r[i]
        if gl == rl:
            continue
        # same tag?
        gp = gl.split("|", 1)
        rp = rl.split("|", 1)
        if gp[0] != rp[0]:
            divs.append((i, "TAG", gl, rl))
        elif norm(gp[1]) != norm(rp[1]):
            divs.append((i, "VAL", gl, rl))
    for i in rorder:
        if i not in g:
            extra.append(i)
    print(f"GNU rows={len(g)} remacs rows={len(r)}")
    print(f"missing(remacs died/skip)={len(missing)} extra={len(extra)} divs={len(divs)}")
    for i in missing[:30]:
        print(f"  MISSING idx={i} GNU={g[i][:160]}")
    for i in extra[:30]:
        print(f"  EXTRA idx={i} remacs={r[i][:160]}")
    for i, k, gl, rl in divs:
        print(f"  DIV {k} idx={i}\n    GNU : {gl[:200]}\n    REM : {rl[:200]}")


if __name__ == "__main__":
    main()
