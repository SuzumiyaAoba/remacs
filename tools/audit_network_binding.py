#!/usr/bin/env python3
"""Inspect the actual bind address of one owned test listener with lsof.

No clients are connected.  The child is terminated and reaped in finally.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FORM = """(let ((s (make-network-process :name "audit-binding" :host "127.0.0.1"
               :family 'ipv4 :server t :service 0 :noquery t)))
  (unwind-protect (progn (message "AUDIT-BOUND") (sleep-for 30))
    (delete-process s)))"""


def inspect(binary, side, directory):
    proc = subprocess.Popen([str(binary), "-Q", "--batch", "--eval", FORM], cwd=ROOT,
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    trace = b""
    try:
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline and proc.poll() is None:
            if select.select([proc.stderr], [], [], 0.1)[0]:
                trace += os.read(proc.stderr.fileno(), 4096)
                if b"AUDIT-BOUND" in trace:
                    break
        if b"AUDIT-BOUND" not in trace:
            raise RuntimeError((side, "No readiness marker", trace))
        result = subprocess.run(["/usr/sbin/lsof", "-a", "-p", str(proc.pid), "-iTCP",
                                 "-sTCP:LISTEN", "-nP", "-Fn"],
                                capture_output=True, timeout=3)
        (directory / f"network-binding.{side}.out").write_bytes(result.stdout)
        (directory / f"network-binding.{side}.err").write_bytes(trace + result.stderr)
        addresses = [line[1:].rsplit(":", 1)[0] for line in result.stdout.decode().splitlines()
                     if line.startswith("n")]
        if result.returncode != 0 or len(addresses) != 1:
            raise RuntimeError((side, "Expected one listener", result))
        return json.dumps(dict(actual_listen_host=addresses[0]), ensure_ascii=False)
    finally:
        if proc.poll() is None:
            proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gnu", required=True)
    parser.add_argument("--remacs", default="target/debug/remacs")
    parser.add_argument("--output-dir", default="tests/audit")
    args = parser.parse_args()
    directory = (ROOT / args.output_dir).resolve()
    path = directory / "evidence.json"
    evidence = json.loads(path.read_text())
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if evidence["head"] != head:
        raise ValueError("HEAD changed; collect a new baseline first")
    values = {}
    runs = []
    for side, binary in (("gnu", Path(args.gnu).resolve()),
                         ("remacs", (ROOT / args.remacs).resolve())):
        values[side] = inspect(binary, side, directory)
        runs.append(dict(group="network_system", side=side, returncode=0,
                         completed=True, timed_out=False,
                         command=["python3", "tools/audit_network_binding.py", "--gnu", args.gnu],
                         stdout_file=f"network-binding.{side}.out",
                         stderr_file=f"network-binding.{side}.err"))
        print(side, values[side], flush=True)
    evidence["cases"] = [c for c in evidence["cases"] if c["group"] != "network_system"]
    evidence["runs"] = [r for r in evidence["runs"] if r["group"] != "network_system"] + runs
    evidence["cases"].append(dict(id="network:actual-loopback-binding", group="network_system",
                                  form=FORM + "\nInspect the child PID with lsof -a -p PID -iTCP -sTCP:LISTEN -nP -Fn; terminate child.",
                                  gnu=values["gnu"], remacs=values["remacs"], differs=values["gnu"] != values["remacs"]))
    evidence["probe_sha256"]["tools/audit_network_binding.py"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    evidence["scope"] = sorted({c["group"] for c in evidence["cases"]})
    evidence["updated_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    path.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
