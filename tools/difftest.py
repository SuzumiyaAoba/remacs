#!/usr/bin/env python3
"""Differential tester: run tests/diff/driver.el under GNU Emacs and remacs,
compare the DIF| row stream (emitted on stderr via `message'), report
divergences.

Usage:
  tools/difftest.py [--only name1,name2] [--timeout SEC] [--no-regen]
                    [--remacs PATH] [--gnu PATH]

Env DIFF_ONLY / DIFF_FROM / DIFF_AUTOSKIP are honoured by the driver.
Rows are keyed by (name, per-name-index) so a crashed/hung run can resume
after the offender (recorded in tests/diff/autoskip-<side>.txt).

Artifacts: tests/diff/{gnu,remacs}.{out,err}, sandbox/, report.txt.
"""
import os, re, shutil, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIFF = os.path.join(ROOT, "tests", "diff")
DRIVER = os.path.join(DIFF, "driver.el")
SANDBOX = os.path.join(DIFF, "sandbox")
KNOWN = os.path.join(DIFF, "known.txt")

ROW = re.compile(r"DIF\|([^|]*)\|(\d+)\|(ok|err|unbound)\|(.*)")
MARK = re.compile(r"DMARK ([^ ]+) (\d+)")

def gen_targets(gnu):
    path = DIFF.replace("\\", "\\\\").replace('"', '\\"')
    el = ("(let ((out nil)) (mapatoms (lambda (s) "
          "(let ((f (symbol-function s))) "
          "(when (and (subrp f) (not (special-form-p s))) "
          "(push (list (symbol-name s) (ignore-errors (subr-arity f))) out))))) "
          "(setq out (sort out (lambda (a b) (string< (car a) (car b))))) "
          f'(with-temp-file "{path}/targets.el" '
          "(insert \";;; -*- lexical-binding: t -*-\\n(setq dif--targets '\") "
          "(prin1 out (current-buffer)) (insert \")\")))")
    subprocess.run([gnu, "-Q", "--batch", "--eval", el],
                   check=True, capture_output=True)

def make_sandbox():
    shutil.rmtree(SANDBOX, ignore_errors=True)
    os.makedirs(os.path.join(SANDBOX, "sub"))
    os.makedirs(os.path.join(SANDBOX, "dir"))
    with open(os.path.join(SANDBOX, "file-a.txt"), "w") as f:
        f.write("alpha beta\ngamma delta epsilon\nzeta\n")
    with open(os.path.join(SANDBOX, "sub", "b.el"), "w") as f:
        f.write('(setq dif-b-loaded "loaded")\n')
    with open(os.path.join(SANDBOX, "dir", ".hidden"), "w") as f:
        f.write("h\n")
    open(os.path.join(SANDBOX, "empty.bin"), "wb").close()
    with open(os.path.join(SANDBOX, "bin.dat"), "wb") as f:
        f.write(bytes(range(256)) + b"\xff\xfe\x00\x1f\x80")
    for src, dst in [("file-a.txt", "link-a"), ("nofile-xyz", "link-broken")]:
        try:
            os.symlink(src, os.path.join(SANDBOX, dst))
        except OSError:
            pass
    with open(os.path.join(SANDBOX, "unreadable.txt"), "w") as f:
        f.write("no\n")
    os.chmod(os.path.join(SANDBOX, "unreadable.txt"), 0)

def last_dmark(path):
    """Return name of the last in-flight call recorded on stderr."""
    try:
        for line in reversed(open(path, errors="replace").readlines()):
            m = MARK.search(line)
            if m:
                return m.group(1)
            if "DIFDONE" in line:
                return None
    except OSError:
        pass
    return None

def done_flag(path):
    try:
        return "DIFDONE" in open(path, errors="replace").read()
    except OSError:
        return False

def autoskip_path(tag):
    return os.path.join(DIFF, f"autoskip-{tag}.txt")

def load_autoskip(tag):
    try:
        return set(l.strip() for l in open(autoskip_path(tag)) if l.strip())
    except OSError:
        return set()

def run_side(binpath, extra_args, tag, timeout, env, resume=True):
    """Run the driver; on hang/crash autoskip the in-flight name and resume."""
    err_p = os.path.join(DIFF, f"{tag}.err")
    out_p = os.path.join(DIFF, f"{tag}.out")
    open(err_p, "w").close()
    open(out_p, "w").close()
    autoskip = set() if not resume else load_autoskip(tag)
    from_name = None
    t0 = time.time()
    attempt = 0
    while True:
        attempt += 1
        make_sandbox()
        env2 = dict(env)
        if autoskip:
            env2["DIFF_AUTOSKIP"] = ",".join(sorted(autoskip))
        if from_name:
            env2["DIFF_FROM"] = from_name
        timed_out = False
        with open(out_p, "ab") as out, open(err_p, "ab") as err, \
             open(os.devnull, "rb") as dn:
            try:
                p = subprocess.run(
                    binpath.split() + extra_args + ["-l", DRIVER],
                    cwd=SANDBOX, stdin=dn, stdout=out, stderr=err,
                    timeout=timeout, env=env2)
                rc = p.returncode
            except subprocess.TimeoutExpired:
                timed_out = True
                rc = -9
        if done_flag(err_p):
            return "DONE", "", time.time() - t0, autoskip, attempt
        offender = last_dmark(err_p)
        if not resume or not offender or offender in autoskip or attempt > 200:
            return ("TIMEOUT" if timed_out else f"EXIT{rc}"), \
                offender or "?", time.time() - t0, autoskip, attempt
        autoskip.add(offender)
        from_name = offender
        with open(autoskip_path(tag), "a") as f:
            f.write(offender + "\n")
        print(f"[difftest] {tag}: autoskip {offender} "
              f"(attempt {attempt})", flush=True)

def parse_rows(path):
    rows = {}
    order = []
    for line in open(path, errors="replace"):
        m = ROW.search(line)
        if m:
            rows[(m.group(1), int(m.group(2)))] = (m.group(3), m.group(4))
            order.append((m.group(1), int(m.group(2))))
    return rows, order

def load_known():
    rules = []
    if os.path.exists(KNOWN):
        for line in open(KNOWN):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            rules.append(line)
    return rules

def is_known(name, payload, rules):
    for r in rules:
        if "|" in r:
            n, pat = r.split("|", 1)
            if name == n and pat in payload:
                return True
        elif name == r:
            return True
    return False

def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="")
    ap.add_argument("--side", default="both",
                    choices=["both", "gnu", "remacs"])
    ap.add_argument("--timeout", type=int, default=900)
    ap.add_argument("--no-regen", action="store_true")
    ap.add_argument("--no-resume", action="store_true")
    ap.add_argument("--remacs", default=os.path.join(ROOT, "target/release/remacs"))
    ap.add_argument("--gnu", default="emacs")
    a = ap.parse_args()

    only = a.only or os.environ.get("DIFF_ONLY", "")
    env = dict(os.environ, TZ="UTC", LC_ALL="C",
               DIFF_DIR=DIFF, EMACSLOADPATH="", EMACSPATH="",
               DIFF_ONLY=only)
    if not a.no_regen:
        gen_targets(a.gnu)

    resume = not a.no_resume and not only
    st, last, dt, gskip, gatt = "SKIP", "", 0, set(), 0
    st2, last2, dt2, rskip, ratt = "SKIP", "", 0, set(), 0
    if a.side in ("both", "gnu"):
        print(f"[difftest] running GNU {a.gnu} ...", flush=True)
        st, last, dt, gskip, gatt = run_side(a.gnu, ["-Q", "--batch"], "gnu",
                                           a.timeout, env, resume)
        print(f"[difftest] GNU {st} in {dt:.0f}s attempts={gatt} "
              f"autoskipped={sorted(gskip)} last={last}", flush=True)
    if a.side in ("both", "remacs"):
        print(f"[difftest] running remacs {a.remacs} ...", flush=True)
        st2, last2, dt2, rskip, ratt = run_side(a.remacs, ["--batch"], "remacs",
                                              a.timeout, env, resume)
        print(f"[difftest] remacs {st2} in {dt2:.0f}s attempts={ratt} "
              f"autoskipped={sorted(rskip)} last={last2}", flush=True)

    grow, gorder = parse_rows(os.path.join(DIFF, "gnu.err"))
    rrow, rorder = parse_rows(os.path.join(DIFF, "remacs.err"))
    rules = load_known()

    divs = {}
    missing_g, missing_r = [], []
    for k in gorder:
        if k not in rrow:
            missing_r.append(k); continue
        if grow[k] != rrow[k]:
            divs.setdefault(k[0], []).append((k, grow[k], rrow[k]))
    for k in rorder:
        if k not in grow:
            missing_g.append(k)

    rep = []
    new = 0
    # remacs-only autoskips are candidate bugs (hangs/crashes GNU survived)
    for nm in sorted(rskip - gskip):
        rep.append(f"DIVERG {nm} (autoskipped: remacs hung/crashed, GNU fine)")
        new += 1
    for nm in sorted(gskip - rskip):
        rep.append(f"KNOWN {nm} (autoskipped: GNU-side hang only)")
    for name in sorted(set(list(divs) +
                         [k[0] for k in missing_r + missing_g])):
        items = divs.get(name, [])
        if not items:
            items = [(k, grow.get(k, ("-", "-")), rrow.get(k, ("MISSING", "-")))
                     for k in missing_r + missing_g if k[0] == name]
        known = all(is_known(name, f"{g[0]}|{g[1]}", rules)
                    and is_known(name, f"{r[0]}|{r[1]}", rules)
                    for _, g, r in items)
        if not known:
            new += 1
        tag = "KNOWN " if known else "DIVERG"
        rep.append(f"{tag} {name} ({len(items)} rows)")
        for k, g, r in items[:8]:
            rep.append(f"    row#{k[1]}")
            rep.append(f"      gnu:    {g[0]} {g[1][:220]}")
            rep.append(f"      remacs: {r[0]} {r[1][:220]}")
    rep.append(f"rows: gnu={len(grow)} remacs={len(rrow)} "
               f"divergent-names={len(divs) + len(set(k[0] for k in missing_r + missing_g))} "
               f"new={new} gnu-status={st} remacs-status={st2}")
    open(os.path.join(DIFF, "report.txt"), "w").write("\n".join(rep) + "\n")
    print("\n".join(rep[:500]))
    if len(rep) > 500:
        print(f"... {len(rep)-500} more lines in tests/diff/report.txt")
    sys.exit(1 if new else 0)

if __name__ == "__main__":
    main()
