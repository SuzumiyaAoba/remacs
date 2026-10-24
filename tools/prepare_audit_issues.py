#!/usr/bin/env python3
"""Render self-contained reproductions and issue drafts from the audit ledger.

This command does not contact GitHub or create issues.
"""
import json
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / "tests/audit"


def printable_observation(value):
    return "".join(f"\\{ord(ch):03o}" if ord(ch) < 32 or ord(ch) == 127 else ch
                   for ch in value)


def setup(group):
    if group in ("external", "hash"):
        return ""
    source = (ROOT / f"tests/audit_{group}.el").read_text()
    if group == "native":
        source = source.split("(defun audit-native-run", 1)[0]
    elif group == "network":
        source = source.split('(audit-network-check "network:pair-control"', 1)[0]
        start = source.index("(defun audit-network-check")
        end = source.index("(defun audit-net-server")
        source = source[:start] + source[end:]
    else:
        before = source.split("(audit-check", 1)[0]
        if group == "files":
            before = source.split("(dolist (kind", 1)[0]
            before += source[source.index("(defun audit-magic-file-handler"):source.index("(dolist (entry")]
        source = before
    return "\n".join(line for line in source.splitlines()
                     if not line.startswith(";;") and not line.startswith("(load ")).strip() + "\n"


def lisp_repro(finding, cases):
    group = cases[0]["group"]
    groups = list(dict.fromkeys(case["group"] for case in cases))
    header = ";;; -*- lexical-binding: t; -*-\n" + "\n".join(setup(g) for g in groups)
    body = []
    for case in cases:
        form = case["form"]
        # The old printer emitted raw non-UTF-8 bytes for this literal.
        if case["id"] == "native:string-byte-kind":
            form = r'(let ((f (audit-native-compile (lambda () "\200\377")))) (list (multibyte-string-p (funcall f)) (string-to-list (funcall f))))'
        elif case["id"] == "native:string-byte-kind-baseline":
            form = r'(list (multibyte-string-p "\200\377") (string-to-list "\200\377"))'
        assert "\ufffd" not in form, case["id"]
        body.append(f";; {case['id']!r}\n"
                    "(let ((print-circle t) (print-escape-newlines t)\n"
                    "      (print-escape-control-characters t))\n"
                    "  (prin1 (condition-case err\n"
                    f"             (list 'ok (eval '{form} t))\n"
                    "           (error (list 'error err))))\n  (terpri))\n")
    if group == "native":
        return header + "(unwind-protect\n    (progn\n" + "\n".join(body) + ")\n  (delete-directory audit-native-dir t))\n"
    return header + "\n".join(body)


def cli_repro(cases):
    statements = []
    for case in cases:
        tail = json.loads(case["form"][5:].split('; fixture.txt=', 1)[0])
        statements.append(f"    argv = {tail!r}\n"
                          "    argv = [arg.replace('<audit-dir>', str(work)) for arg in argv]\n"
                          "    result = subprocess.run([binary, *argv], cwd=work,\n"
                          "                            capture_output=True, timeout=30)\n"
                          f"    print({case['id']!r}, result.returncode, repr(result.stdout.decode()))\n")
    return ("import pathlib, subprocess, sys, tempfile\n"
            "binary = str(pathlib.Path(sys.argv[1]).resolve())\n"
            "with tempfile.TemporaryDirectory(prefix='remacs-repro-') as tmp:\n"
            "    work = pathlib.Path(tmp)\n"
            "    (work / 'fixture.txt').write_text('fixture-text\\n')\n"
            "    (work / 'lib').mkdir()\n"
            "    (work / 'lib' / 'audit-cli-library.el').write_text(\n"
            "        \"(defvar audit-cli-library-value 42)\\n(provide 'audit-cli-library)\\n\")\n"
            + "\n".join(statements))


def binding_repro():
    source = (ROOT / "tools/audit_network_binding.py").read_text()
    code = source[source.index('FORM = """'):source.index("\ndef main():")]
    return ("import json, os, pathlib, select, subprocess, sys, tempfile, time\n"
            "from pathlib import Path\nROOT = Path.cwd()\n" + code +
            "\nwith tempfile.TemporaryDirectory(prefix='remacs-bind-repro-') as tmp:\n"
            "    print(inspect(Path(sys.argv[1]).resolve(), 'repro', Path(tmp)))\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence")
    parser.add_argument("--ids", nargs="+")
    args = parser.parse_args()
    ledger = json.loads((DIRECTORY / "findings.json").read_text())
    evidence_path = ROOT / args.evidence if args.evidence else DIRECTORY / ledger["evidence_file"]
    evidence = json.loads(evidence_path.read_text())
    case_map = {c["id"]: c for c in evidence["cases"]}
    out = DIRECTORY / "issues"
    out.mkdir(exist_ok=True)
    prepared = 0
    for finding in ledger["findings"]:
        if args.ids and finding["id"] not in args.ids:
            continue
        cases = [case_map[c] for c in finding["cases"]]
        group = cases[0]["group"]
        if group == "cli":
            repro, extension, language = cli_repro(cases), "py", "python"
        elif group == "network_system":
            repro, extension, language = binding_repro(), "py", "python"
        else:
            repro, extension, language = lisp_repro(finding, cases), "el", "elisp"
        filename = f"{finding['id']}.{extension}"
        (out / filename).write_text(repro)
        command = (f"python3 {filename} ./target/debug/remacs\n"
                   f"python3 {filename} /path/to/emacs-31.1") if extension == "py" else (
                       f"./target/debug/remacs -Q --batch -l {filename}\n"
                       f"/path/to/emacs-31.1 -Q --batch -l {filename}")
        notes = []
        if group == "treesit":
            notes.append("前提: tree-sitter の JSON 文法を `~/.emacs.d/tree-sitter/` に配置する。")
        if group == "native":
            notes.append("前提: GNU 側は native compilation 対応ビルド、remacs 側は利用可能な C コンパイラ。生成物は一時ディレクトリへ保存し、終了時に削除する。")
        if group in ("network", "network_system"):
            notes.append("前提: ローカルソケットの bind/connect が許可された macOS 環境。")
        if finding["id"] == "AUD-003":
            notes.append("remacs は停止しないため、20 秒などの上限を付けてプロセスを終了する。GNU 側は直ちに完了する。")
        if finding["id"] == "AUD-045":
            notes.append("remacs の stderr に Rust panic が出力される。Lisp の error handler には戻らず、終了コードは 0 となる。")
        if any(any(ord(ch) < 32 or ord(ch) == 127 for ch in case[side])
               for case in cases for side in ("gnu", "remacs")):
            notes.append("結果に含まれる制御文字は、読みやすいよう 8 進エスケープで表示する。")
        observations = []
        for case in cases:
            observations.append(f"`{case['id']!r}`\n\n```text\nGNU Emacs 31.1: {printable_observation(case['gnu'])}\nremacs:         {printable_observation(case['remacs'])}\n```")
        source, line = finding["source"].rsplit(":", 1)
        body = (f"{finding['description']}\n\n"
                f"対象: `{evidence['head']}` / macOS / GNU Emacs 31.1 との同一再現コードによる比較。\n\n"
                "## 再現手順\n\n" + "\n\n".join(notes) +
                f"\n\n以下を `{filename}` として保存する。\n\n```{language}\n{repro}```\n\n"
                f"```sh\n{command}\n```\n\n## 期待結果と実際の結果\n\n" +
                "\n\n".join(observations) +
                f"\n\n## 調査箇所\n\n原因候補: [{source}:{line}]"
                f"(https://github.com/SuzumiyaAoba/remacs/blob/{evidence['head']}/{source}#L{line})。"
                "修正はまだ行っていない。\n\n"
                f"<!-- remacs-audit-id: {finding['id']} -->\n")
        (out / f"{finding['id']}.md").write_text(body)
        prepared += 1
    print(f"Prepared {prepared} self-contained reproductions and issue drafts in {out}")


if __name__ == "__main__":
    main()
