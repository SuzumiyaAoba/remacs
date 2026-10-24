#!/usr/bin/env python3
"""Validate findings against saved observations and render a Japanese text report."""
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / "tests/audit"


def main():
    ledger = json.loads((DIRECTORY / "findings.json").read_text())
    evidence = json.loads((DIRECTORY / ledger["evidence_file"]).read_text())
    cases = {c["id"]: c for c in evidence["cases"]}
    issues = json.loads((DIRECTORY / ledger["existing_issues_file"]).read_text())
    findings = ledger["findings"]
    assert len({f["id"] for f in findings}) == len(findings)
    assert evidence["head"].startswith(ledger["baseline"])
    assert len(issues) == ledger["deduplication"]["issue_count"]
    covered = set()
    for finding in findings:
        assert finding["cases"] and any(cases[c]["differs"] for c in finding["cases"]), finding["id"]
        for case_id in finding["cases"]:
            case = cases[case_id]
            assert case["form"] and case["gnu"] and case["remacs"], case_id
            covered.add(case_id)
        path, line = finding["source"].rsplit(":", 1)
        assert 1 <= int(line) <= len((ROOT / path).read_text().splitlines()), finding["source"]
    excluded = {c["case"] for c in ledger["excluded_or_unresolved"]}
    assert not ({c["id"] for c in evidence["cases"] if c["differs"]} - covered - excluded)
    for probe, digest in evidence["probe_sha256"].items():
        import hashlib
        assert hashlib.sha256((ROOT / probe).read_bytes()).hexdigest() == digest, probe

    counts = Counter(f["area"] for f in findings)
    priorities = Counter(f["priority"] for f in findings)
    output_dir = str((DIRECTORY / ledger["evidence_file"]).parent.relative_to(ROOT))
    runtime_groups = " ".join(sorted({c["group"] for c in cases.values()} - {"cli", "network_system"}))
    lines = [
        f"Remacs バグ調査 — 新規 {len(findings)} 件（調査継続中）",
        f"Issue 登録: {sum(bool(f.get('issue_url')) for f in findings)} / {len(findings)} 件。",
        f"別途、実GUI・テスト実行の問題 {ledger.get('manual_issue_count', 0)} 件は current/full-suite/report.txt と manual-issues.json に記録。",
        f"対象 HEAD: {evidence['head']}",
        f"最終測定日時 UTC: {evidence.get('updated_at', evidence['recorded_at'])}",
        f"重複照合の基準一覧: open/closed 合計 {len(issues)} 件。追加の照合結果は下記参照。",
        f"差分検査: {len(cases)} ケース / 差分 {sum(c['differs'] for c in cases.values())} 行。",
        "差分行を原因・修正箇所でまとめ、GNU 側の挙動が曖昧なケース等を除外した件数。",
        f"領域別: {dict(counts)} / 暫定優先度: {dict(priorities)}",
        "全領域の調査完了を示す結果ではない。未検査領域は末尾を参照。",
        "",
        "再実行（GNU Emacs 31.1 の生バイナリを指定）:",
        f"  rtk proxy python3 tools/audit_runtime.py --gnu /path/to/emacs-31.1 --output-dir {output_dir} --groups {runtime_groups} --issues-json /path/to/issues.json",
        f"  rtk proxy python3 -B tools/audit_cli.py --gnu /path/to/emacs-31.1 --output-dir {output_dir}",
        f"  rtk proxy python3 -B tools/audit_network_binding.py --gnu /path/to/emacs-31.1 --output-dir {output_dir}",
        "  rtk proxy python3 tools/render_audit_report.py",
        "tree-sitter 検査には ~/.emacs.d/tree-sitter の JSON 文法ライブラリが必要。実行時のファイル情報は evidence.json に保存。",
        "network 検査にはローカルの bind/connect が可能な実行環境、実際の bind 先検査には /usr/sbin/lsof が必要。",
        "再現式で使う audit-native-compile 等の補助関数は tests/audit_native.el に定義。",
        "循環 equal と巨大待ち時間は個別プロセスで 20 秒上限。通常の検査は各プロセス 60 秒上限。CLI は各起動 30 秒上限。",
        "",
        "重複の確認:",
        *["  " + note for note in ledger["deduplication"]["notes"]],
        "",
    ]
    inventory_path = DIRECTORY / "inventory-summary.json"
    if inventory_path.exists():
        inventory = json.loads(inventory_path.read_text())
        lines.extend([
            f"起動時の関数名棚卸し: GNU {inventory['gnu_count']}、remacs {inventory['remacs_count']}、共通 {inventory['shared_count']}。",
            "これは fboundp の名前一覧であり、呼び出しの動作検証・網羅率ではない。詳細は inventory.*.json を参照。",
            "",
        ])
    for finding in findings:
        case = cases[finding["cases"][0]]
        lines.extend([
            f"{finding['id']} [{finding['priority']}] {finding['title']}",
            "  Issue: " + finding.get("issue_url", "未登録"),
            "  " + finding["description"],
            "  原因候補: " + finding["source"],
            "  再現式: " + case["form"],
            "  GNU: " + case["gnu"],
            "  remacs: " + case["remacs"],
            "  関連ケース: " + ", ".join(repr(c) for c in finding["cases"]),
            "",
        ])
    lines.extend(["確定一覧から除外した差分:", *["  " + e["case"] + ": " + e["reason"]
                                                    for e in ledger["excluded_or_unresolved"]], ""])
    qa = DIRECTORY / "qa-summary.json"
    if qa.exists():
        summary = json.loads(qa.read_text())
        lines.extend(["既存 QA の確認:", *["  " + line for line in summary["notes"]], ""])
    lines.extend(["残る調査範囲:", *["  " + item for item in ledger["remaining_scope"]], ""])
    (DIRECTORY / "report.txt").write_text("\n".join(lines))
    print(f"Verified {len(findings)} findings against {len(cases)} cases; wrote tests/audit/report.txt")


if __name__ == "__main__":
    main()
