#!/usr/bin/env python3
"""Compare bounded, headless CLI scenarios; append them to audit evidence."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gnu", required=True)
    parser.add_argument("--remacs", default="target/debug/remacs")
    parser.add_argument("--output-dir", default="tests/audit")
    args = parser.parse_args()
    directory = (ROOT / args.output_dir).resolve()
    evidence = json.loads((directory / "evidence.json").read_text())
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if evidence["head"] != head:
        raise ValueError("HEAD changed; collect a fresh baseline first")
    binaries = {"gnu": Path(args.gnu).resolve(), "remacs": (ROOT / args.remacs).resolve()}
    evidence["cases"] = [c for c in evidence["cases"] if c["group"] != "cli"]
    evidence["runs"] = [r for r in evidence["runs"] if r["group"] != "cli"]
    now = datetime.datetime.now(datetime.timezone.utc).isoformat()
    env = dict(os.environ, TZ="UTC", LC_ALL="C")
    with tempfile.TemporaryDirectory(prefix="remacs-audit-cli-") as tmp:
        work = Path(tmp)
        fixture = work / "fixture.txt"
        fixture.write_text("fixture-text\n")
        library = work / "lib"
        library.mkdir()
        (library / "audit-cli-library.el").write_text(
            ";;; -*- lexical-binding: t -*-\n"
            "(defvar audit-cli-library-value 42)\n(provide 'audit-cli-library)\n")
        load_expr = "(condition-case e (progn (require 'audit-cli-library) (princ audit-cli-library-value)) (error (prin1 (car e))))"
        tests = {
            "cli:eval-control": ["--eval", "(princ (+ 1 2))"],
            "cli:eval-multiple-forms": ["--eval", "(princ 1) (princ 2)"],
            "cli:file-before-eval": [str(fixture), "--eval", "(princ (buffer-string))"],
            "cli:file-after-eval-control": ["--eval", "(princ (buffer-string))", str(fixture)],
            "cli:funcall-consumed-arguments": ["--eval", "(defun audit-cli-consume () (setq command-line-args-left nil))", "-f", "audit-cli-consume", "--eval", "(princ 123)"],
            "cli:end-of-options": ["--", "--eval", "(princ 123)"],
            "cli:load-path-short": ["-L", str(library), "--eval", load_expr],
            "cli:load-path-long": ["--directory", str(library), "--eval", load_expr],
            "cli:single-hyphen-eval": ["-eval", "(princ 7)"],
            "cli:equals-eval": ["--eval=(princ 7)"],
            "cli:help": ["--help"],
        }
        observations = {key: {} for key in tests}
        for side, binary in binaries.items():
            stdout_records, stderr_records = [], []
            for case_id, tail in tests.items():
                argv = [str(binary), "-Q", "--batch", *tail]
                result = subprocess.run(argv, cwd=work, env=env, stdin=subprocess.DEVNULL,
                                        capture_output=True, timeout=30)
                out = result.stdout.decode("utf-8", errors="replace").replace(tmp, "<audit-dir>")
                err = result.stderr.decode("utf-8", errors="replace").replace(tmp, "<audit-dir>")
                value = dict(exit=result.returncode, stdout=out)
                if case_id == "cli:help":
                    value = dict(exit=result.returncode, has_usage="Usage:" in out)
                observations[case_id][side] = json.dumps(value, ensure_ascii=False)
                stdout_records.append(json.dumps(dict(id=case_id, argv=[str(x).replace(tmp, "<audit-dir>") for x in argv[1:]], **value), ensure_ascii=False))
                stderr_records.append(case_id + ":\n" + err)
                print(f"{side} {case_id}: {observations[case_id][side]}", flush=True)
            (directory / f"cli.{side}.out").write_text("\n".join(stdout_records) + "\n")
            (directory / f"cli.{side}.err").write_text("\n".join(stderr_records) + "\n")
            evidence["runs"].append(dict(group="cli", side=side, completed=True,
                                          returncode=0, timed_out=False, recorded_at=now,
                                          command=["python3", "tools/audit_cli.py", "--gnu", str(binaries["gnu"])],
                                          stdout_file=f"cli.{side}.out", stderr_file=f"cli.{side}.err"))
        for case_id, tail in tests.items():
            gnu, remacs = observations[case_id]["gnu"], observations[case_id]["remacs"]
            form = "argv=" + json.dumps(["-Q", "--batch", *tail], ensure_ascii=False).replace(tmp, "<audit-dir>")
            if case_id == "cli:file-before-eval" or case_id == "cli:file-after-eval-control":
                form += '; fixture.txt="fixture-text\\n"'
            evidence["cases"].append(dict(id=case_id, group="cli", form=form,
                                          gnu=gnu, remacs=remacs, differs=gnu != remacs))
    evidence["updated_at"] = now
    evidence["scope"] = sorted({c["group"] for c in evidence["cases"]})
    evidence["probe_sha256"]["tools/audit_cli.py"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (directory / "evidence.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
