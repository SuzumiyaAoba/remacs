#!/usr/bin/env python3
"""Drive an owned terminal editor and report its saved bytes."""
import argparse
import errno
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
from pathlib import Path


def plain(data):
    data = re.sub(rb"\x1b\].*?(?:\x07|\x1b\\)", b"", data, flags=re.S)
    data = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", data)
    data = re.sub(rb"\x1b[ -/]+[0-~]", b"", data)
    return data.decode("utf-8", "replace")


def run(binary, case):
    with tempfile.TemporaryDirectory(prefix="remacs-terminal-") as root:
        target = Path(root) / "sample.dat"
        target.write_bytes(b"")
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))

        def setup():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        environment = os.environ.copy()
        environment["TERM"] = "xterm-256color"
        editor_arguments = [binary, "-Q", "-nw", str(target)]
        startup_form = '(insert (if noninteractive "wrong-batch" "startup"))'
        if case == "startup-eval":
            editor_arguments += ["--eval", startup_form]
        elif case == "startup-load":
            startup_file = Path(root) / "startup.el"
            startup_file.write_text(startup_form + "\n")
            editor_arguments += ["-l", str(startup_file)]
        elif case == "startup-funcall":
            editor_arguments += ["-f", "newline"]
        child = subprocess.Popen(
            editor_arguments,
            stdin=slave, stdout=slave, stderr=slave,
            preexec_fn=setup, close_fds=True, env=environment,
        )
        os.close(slave)
        transcript = bytearray()
        deadline = time.monotonic() + 120

        def pump():
            if select.select([master], [], [], 0.02)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                transcript.extend(chunk)
                if len(transcript) > 4 * 1024 * 1024:
                    del transcript[:len(transcript) - 4 * 1024 * 1024]

        def wait_for(predicate, description):
            while not predicate():
                pump()
                status = child.poll()
                if status is not None:
                    raise AssertionError(f"editor exited ({status}) before {description}: {plain(transcript)[-3000:]}")
                if time.monotonic() >= deadline:
                    raise AssertionError(f"timeout waiting for {description}: {plain(transcript)[-3000:]}")

        def send(data):
            while data:
                sent = os.write(master, data)
                data = data[sent:]

        try:
            wait_for(lambda: b"\x1b[" in transcript and "sample.dat" in plain(transcript), "first frame")
            marker = "TTY-READY"
            ready_form = "(message (string " + " ".join(str(ord(c)) for c in marker) + "))"
            configuration = "(progn (when noninteractive (error \"Editor started as batch\")) (setq-local require-final-newline nil) "
            if case == "controls":
                configuration += "(dolist (code '(20 21 22 23 28 29 30 31)) (global-set-key (vector code) (eval (list 'lambda nil '(interactive) (list 'insert (format \"<%d>\" code)))))) "
            configuration += ready_form + ")"
            send(b"\x1b:" + configuration.encode() + b"\r")
            wait_for(lambda: marker in plain(transcript), "configuration result")
            if case == "controls":
                send(bytes([28, 29, 30, 31]))
            elif case == "undo":
                send(b"abc\x01\x05X\x1f")
            send(b"\x18\x13")
            wait_for(lambda: target.stat().st_size > 0, "saved file")
            saved = target.read_bytes()
            send(b"\x18\x03")
            while child.poll() is None:
                pump()
                if time.monotonic() >= deadline:
                    raise AssertionError(f"editor did not quit: {plain(transcript)[-3000:]}")
            assert child.returncode == 0, (child.returncode, plain(transcript)[-3000:])
            return {"status": child.returncode, "frame": True, "saved": saved.hex()}
        finally:
            if child.poll() is None:
                # The unreaped session leader still owns this PID/group.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=5)
            os.close(master)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--case", choices=[
        "controls", "undo", "startup-eval", "startup-load", "startup-funcall",
    ], required=True)
    arguments = parser.parse_args()
    print(json.dumps(run(arguments.binary, arguments.case)))
