#!/usr/bin/env python3
"""Bounded headless PTY checks with explicit terminal sizes and cleanup."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import termios
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gnu", required=True)
    parser.add_argument("--remacs", default="target/debug/remacs")
    parser.add_argument("--output-dir", default="tests/audit/pty")
    args = parser.parse_args()
    directory = (ROOT / args.output_dir).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    results = []
    for side, binary in (("gnu", Path(args.gnu).resolve()),
                         ("remacs", (ROOT / args.remacs).resolve())):
        for label, rows, cols in (("sized", 24, 80), ("zero", 0, 0)):
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

            def setup():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            proc = subprocess.Popen([str(binary), "-Q", "-nw"], stdin=slave, stdout=slave,
                                    stderr=slave, env=dict(os.environ, TERM="xterm-256color"),
                                    preexec_fn=setup)
            os.close(slave)
            output, seen, started = b"", False, time.monotonic()
            try:
                while time.monotonic() - started < 20 and proc.poll() is None:
                    if select.select([master], [], [], 0.1)[0]:
                        try:
                            chunk = os.read(master, 65536)
                        except OSError:
                            break
                        if not chunk:
                            break
                        output += chunk
                        if b"scratch" in re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", output):
                            seen = True
                            break
                before = proc.poll()
                if before is None:
                    try:
                        os.write(master, b"\x18\x03")
                    except OSError:
                        pass
                    until = time.monotonic() + 3
                    while proc.poll() is None and time.monotonic() < until:
                        if select.select([master], [], [], 0.1)[0]:
                            try:
                                output += os.read(master, 65536)
                            except OSError:
                                break
                if proc.poll() is None:
                    proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
                os.close(master)
            path = directory / f"{label}.{side}.out"
            path.write_bytes(output)
            record = dict(side=side, size=[rows, cols], frame_seen=seen,
                          exit_before_quit=before, exit=proc.returncode,
                          seconds=round(time.monotonic() - started, 2),
                          bytes=len(output), stdout_file=path.name,
                          scratch_in_raw_bytes=b"scratch" in output,
                          scratch_after_ansi_removal=b"scratch" in re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", output),
                          preview=output[:250].decode("utf-8", errors="replace"))
            results.append(record)
            print(record, flush=True)
    (directory / "comparison.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
