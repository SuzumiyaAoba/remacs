#!/usr/bin/env python3
"""Publish explicitly selected, reviewed audit drafts and record each URL.

Stops after any uncertain response; a later invocation reconciles existing
GitHub markers before considering another creation.
"""
import argparse
import datetime
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / "tests/audit"
REPOSITORY = "SuzumiyaAoba/remacs"


def gh(*args):
    return subprocess.check_output(["rtk", "proxy", "gh", *args], cwd=ROOT, text=True, timeout=40)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ids", nargs="+", required=True)
    args = parser.parse_args()
    path = DIRECTORY / "findings.json"
    ledger = json.loads(path.read_text())
    findings = {f["id"]: f for f in ledger["findings"]}
    assert set(args.ids) <= findings.keys()
    existing = json.loads(gh("issue", "list", "--repo", REPOSITORY, "--state", "all",
                             "--limit", "1000", "--json", "number,title,body,url,state"))
    for case_id in args.ids:
        finding = findings[case_id]
        marker = f"<!-- remacs-audit-id: {case_id} -->"
        matches = [x for x in existing if marker in x["body"] or x["title"] == finding["title"]]
        assert len(matches) <= 1, (case_id, "Multiple existing matches; inspect manually")
        if matches:
            url = matches[0]["url"]
        elif finding.get("issue_url"):
            raise RuntimeError(f"Recorded issue missing from GitHub listing: {case_id}")
        else:
            draft = DIRECTORY / "issues" / f"{case_id}.md"
            assert marker in draft.read_text()
            url = gh("issue", "create", "--repo", REPOSITORY, "--title", finding["title"],
                     "--body-file", str(draft)).strip()
            assert url.startswith(f"https://github.com/{REPOSITORY}/issues/")
            existing.append(dict(title=finding["title"], body=marker, url=url))
        finding["issue_url"] = url
        finding["issue_number"] = int(url.rsplit("/", 1)[1])
        finding["issue_recorded_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        path.write_text(json.dumps(ledger, ensure_ascii=False, indent=2) + "\n")
        print(case_id, url, flush=True)


if __name__ == "__main__":
    main()
