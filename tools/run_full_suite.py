#!/usr/bin/env python3
"""Run every compiled Rust test target, recording failures without stopping."""
import argparse
import concurrent.futures
import datetime
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FINISHED = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$", re.MULTILINE)


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    temporary.replace(path)


def run(command, log, env, timeout):
    started = time.monotonic()
    deadline = started + timeout
    next_progress = started + 30
    timed_out = False
    last_finished = started
    finished_count = 0
    with log.open("w") as output:
        proc = subprocess.Popen(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                stdout=output, stderr=subprocess.STDOUT,
                                start_new_session=True)
        try:
            while proc.poll() is None:
                now = time.monotonic()
                if now >= deadline:
                    timed_out = True
                    os.killpg(proc.pid, signal.SIGKILL)
                    break
                if now >= next_progress:
                    rows = FINISHED.findall(log.read_text(errors="replace"))
                    if len(rows) != finished_count:
                        finished_count = len(rows)
                        last_finished = now
                    elif now - last_finished >= 180:
                        timed_out = True
                        os.killpg(proc.pid, signal.SIGKILL)
                        break
                    print(f"{log.stem}: {len(rows)} finished, {now-started:.0f}s", flush=True)
                    next_progress = now + 30
                time.sleep(1)
            proc.wait()
        finally:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
    rows = FINISHED.findall(log.read_text(errors="replace"))
    return dict(command=command, returncode=proc.returncode, timed_out=timed_out,
                seconds=round(time.monotonic()-started, 2), log=log.name,
                results=[dict(name=n, status=s) for n, s in rows])


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--output-dir", default="tests/audit/current/full-suite")
    ap.add_argument("--gnu", required=True)
    ap.add_argument("--dbus-daemon")
    ap.add_argument("--test-threads", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=3600)
    ap.add_argument("--isolated-timeout", type=int, default=120)
    ap.add_argument("--resume", action="store_true")
    args = ap.parse_args()
    directory = (ROOT / args.output_dir).resolve()
    build = json.loads((directory / "build-summary.json").read_text())
    assert build["returncode"] == 0
    env = dict(os.environ, GNU_EMACS=args.gnu)
    daemon = None
    try:
        if args.dbus_daemon:
            config = Path(args.dbus_daemon).parent.parent / "share/dbus-1/session.conf"
            daemon = subprocess.Popen([args.dbus_daemon, f"--config-file={config}", "--nofork",
                                       "--nopidfile", "--print-address=1"],
                                      stdout=subprocess.PIPE,
                                      stderr=(directory / "dbus-daemon.log").open("w"),
                                      text=True)
            address = daemon.stdout.readline().strip()
            assert address and daemon.poll() is None, "Session bus did not start"
            env["DBUS_SESSION_BUS_ADDRESS"] = address
        inventory = []
        for target in build["artifacts"]:
            label = "-".join(target["kind"]) + "-" + target["name"]
            def listing(extra):
                p = subprocess.run([target["executable"], "--list", "--format", "terse", *extra],
                                   cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
                assert p.returncode == 0, (label, p.stderr)
                return [s[:-6] for s in p.stdout.splitlines() if s.endswith(": test")]
            inventory.append(dict(**target, label=label, tests=listing([]), ignored=listing(["--ignored"])))
        save(directory / "inventory.json", inventory)
        total = sum(len(t["tests"]) for t in inventory)
        print(f"Inventory: {len(inventory)} targets, {total} tests; session bus={bool(daemon)}", flush=True)
        path = directory / "rust-results.json"
        state = json.loads(path.read_text()) if args.resume and path.exists() else dict(targets=[])
        state.update(started_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                     total_tests=total, session_bus=bool(daemon), complete=False)
        completed = {r["label"] for r in state["targets"] if r.get("complete") or not r["timed_out"]}
        priority = {"lib-remacs": 0, "bin-remacs": 1, "test-qa": 2}
        for target in sorted(inventory, key=lambda t: (priority.get(t["label"], 3), t["label"])):
            label = target["label"]
            if label in completed:
                continue
            print(f"Starting {label}: {len(target['tests'])} tests", flush=True)
            command = [target["executable"], "--test-threads", str(args.test_threads)]
            prior = next((r for r in state["targets"] if r["label"] == label), None)
            result = prior if prior else run(command, directory / f"{label}.log", env, args.timeout)
            result["label"] = label
            result["expected_tests"] = len(target["tests"])
            done_names = {r["name"] for r in result["results"]}
            pending = set(target["tests"]) - done_names - set(target["ignored"])
            result.setdefault("isolated_retries", [])
            pending -= {r["name"] for r in result["isolated_retries"]}
            if pending:
                def retry_one(job):
                    index, name = job
                    print(f"Retrying unreported test {label}::{name}", flush=True)
                    retry = run([target["executable"], name, "--exact", "--nocapture"],
                                directory / f"{label}-remaining-{index}.log", env, args.isolated_timeout)
                    retry["name"] = name
                    return retry
                with concurrent.futures.ThreadPoolExecutor(max_workers=args.test_threads) as pool:
                    result["isolated_retries"].extend(pool.map(retry_one, enumerate(sorted(pending))))
            result["complete"] = True
            state["targets"] = [t for t in state["targets"] if t["label"] != label]
            state["targets"].append(result)
            save(path, state)
            counts = {s: sum(r["status"] == s for r in result["results"])
                      for s in ("ok", "FAILED", "ignored")}
            print(f"Finished {label}: rc={result['returncode']}, {counts}", flush=True)
        state["complete"] = True
        state["finished_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        save(path, state)
        print("All Rust targets finished; failures remain recorded for analysis.", flush=True)
    finally:
        if daemon is not None:
            daemon.terminate()
            daemon.wait(timeout=5)


if __name__ == "__main__":
    main()
