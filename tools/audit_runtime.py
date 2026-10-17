#!/usr/bin/env python3
"""Save reproducible GNU/remacs differential evidence without posting issues.

Usage: python3 tools/audit_runtime.py --gnu /path/to/emacs-31.1 \
           --issues-json /tmp/remacs-audit-issues.json
Existing issue bodies are saved as the deduplication baseline.  A complete
run is evidence for these probes only; it does not prove the absence of bugs.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def lisp_string(text):
    escapes = {"n": "\n", "r": "\r", "t": "\t", "b": "\b",
               "f": "\f", "a": "\a", "v": "\v", "e": "\x1b"}

    def unescape(match):
        value = match[1]
        return chr(int(value, 8)) if value[0] in "01234567" else escapes.get(value, value)

    if not (text.startswith('"') and text.endswith('"')):
        raise ValueError(f"Expected printed string ID: {text!r}")
    return re.sub(r"\\([0-7]{1,3}|.)", unescape, text[1:-1])


def rows(stdout, prefix):
    result = {}
    for line in stdout.split("\n"):
        if line.startswith(prefix + "|"):
            _, printed_id, value = line.split("|", 2)
            result[lisp_string(printed_id)] = value
    return result


def run(binary, group, output_dir, tag, timeout, only=None, skip=None):
    env = dict(os.environ, TZ="UTC", LC_ALL="C", REMACS_AUDIT_FORMS="1")
    env.pop("REMACS_AUDIT_ONLY", None)
    env.pop("REMACS_AUDIT_SKIP", None)
    if only:
        env["REMACS_AUDIT_ONLY"] = only
    if skip:
        env["REMACS_AUDIT_SKIP"] = skip
    command = [str(binary), "-Q", "--batch", "-l", f"tests/audit_{group}.el"]
    started = datetime.datetime.now(datetime.timezone.utc)
    try:
        p = subprocess.run(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                           capture_output=True, timeout=timeout)
        out, err, returncode, timed_out = p.stdout, p.stderr, p.returncode, False
    except subprocess.TimeoutExpired as error:
        out, err, returncode, timed_out = error.stdout or b"", error.stderr or b"", None, True
    stem = f"{group}.{tag}"
    (output_dir / (stem + ".out")).write_bytes(out)
    (output_dir / (stem + ".err")).write_bytes(err)
    stdout = out.decode("utf-8", errors="replace")
    elapsed = (datetime.datetime.now(datetime.timezone.utc) - started).total_seconds()
    metadata = dict(command=command, only=only, skip=skip, returncode=returncode,
                    timed_out=timed_out, timeout_seconds=timeout,
                    elapsed_seconds=elapsed, completed="AUDIT-DONE\n" in stdout,
                    stdout_file=stem + ".out", stderr_file=stem + ".err")
    print(f"{group} {tag}: rc={returncode}, timeout={timed_out}, done={metadata['completed']}",
          flush=True)
    return metadata, rows(stdout, "AUDIT"), rows(stdout, "FORM")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gnu", required=True)
    parser.add_argument("--remacs", default="target/debug/remacs")
    parser.add_argument("--output-dir", default="tests/audit")
    parser.add_argument("--issues-json")
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--groups", nargs="+", default=["external", "hash", "native", "concurrency", "process", "treesit", "files", "network"])
    parser.add_argument("--append", action="store_true",
                        help="replace selected groups, preserving evidence for other groups at the same HEAD")
    args = parser.parse_args()
    output_dir = (ROOT / args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    binaries = {"gnu": Path(args.gnu).resolve(), "remacs": (ROOT / args.remacs).resolve()}
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    now = datetime.datetime.now(datetime.timezone.utc).isoformat()
    path = output_dir / "evidence.json"
    if args.append and path.exists():
        evidence = json.loads(path.read_text())
        if evidence["head"] != head:
            raise ValueError("HEAD changed; use a new output directory or re-run all groups")
        evidence["cases"] = [c for c in evidence["cases"] if c["group"] not in args.groups]
        evidence["runs"] = [r for r in evidence["runs"] if r["group"] not in args.groups]
        evidence["updated_at"] = now
    else:
        evidence = dict(recorded_at=now, head=head, exhaustive=False, runs=[], cases=[])
    for group in args.groups:
        if not re.fullmatch(r"[a-z_]+", group):
            raise ValueError(f"Invalid group: {group}")
        sides, forms = {}, {}
        for side, binary in binaries.items():
            skipped = {"hash": "equal:cycles", "process": "process:timeout-overflow"}.get(group)
            metadata, values, source = run(binary, group, output_dir, side, args.timeout,
                                           skip=skipped if side == "remacs" else None)
            metadata.update(group=group, side=side)
            evidence["runs"].append(metadata)
            sides[side] = values
            forms.update(source)
            if metadata["timed_out"] or metadata["returncode"] != 0 or not metadata["completed"]:
                raise RuntimeError(f"Incomplete ordinary probe run: {group} {side}; inspect logs")
        isolated = {"hash": ("equal:cycles", "remacs-cycle"),
                    "process": ("process:timeout-overflow", "remacs-overflow")}.get(group)
        if isolated:
            case_id, tag = isolated
            metadata, values, source = run(binaries["remacs"], group, output_dir,
                                           tag, 20, only=case_id)
            metadata.update(group=group, side="remacs", isolated=True)
            evidence["runs"].append(metadata)
            forms.update(source)
            sides["remacs"].update(values)
            if metadata["timed_out"]:
                sides["remacs"][case_id] = "TIMEOUT after 20 seconds (process terminated)"
            elif not metadata["completed"] and "panicked at" in (output_dir / metadata["stderr_file"]).read_text():
                sides["remacs"][case_id] = f"Rust panic; process exit {metadata['returncode']}; AUDIT-DONE missing"
        for case_id in sorted(set(sides["gnu"]) | set(sides["remacs"])):
            g, r = sides["gnu"].get(case_id), sides["remacs"].get(case_id)
            evidence["cases"].append(dict(id=case_id, group=group, form=forms.get(case_id),
                                          gnu=g, remacs=r, differs=g != r))
        probe = ROOT / f"tests/audit_{group}.el"
        evidence.setdefault("probe_sha256", {})[str(probe.relative_to(ROOT))] = hashlib.sha256(probe.read_bytes()).hexdigest()
        if group in ("concurrency", "process", "treesit", "network", "files"):
            helper = ROOT / "tests/audit_helpers.el"
            evidence["probe_sha256"][str(helper.relative_to(ROOT))] = hashlib.sha256(helper.read_bytes()).hexdigest()
    evidence["scope"] = sorted({c["group"] for c in evidence["cases"]})
    if args.issues_json:
        issues = json.loads(Path(args.issues_json).read_text())
        (output_dir / "existing-issues.json").write_text(json.dumps(issues, ensure_ascii=False, indent=2) + "\n")
        evidence["existing_issue_count"] = len(issues)
        evidence["issue_baseline_file"] = "existing-issues.json"
    (output_dir / "evidence.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    print(f"Saved {len(evidence['cases'])} cases ({sum(c['differs'] for c in evidence['cases'])} differing rows).")


if __name__ == "__main__":
    main()
