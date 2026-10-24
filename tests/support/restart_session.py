#!/usr/bin/env python3
"""Restart an owned terminal editor and inspect its restored termios state."""
import errno
import fcntl
import json
import os
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path

from terminal_session import plain


def run(binary, script, marker, arguments):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    # Darwin revokes the controlling slave when its session leader exits.
    # The master still exposes the same termios state after that revocation.
    original = termios.tcgetattr(master)

    def setup():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    environment = os.environ.copy()
    environment.pop("REMACS_NO_ELC", None)
    environment.pop("EMACSLOADPATH", None)
    environment["TERM"] = "xterm-256color"
    child = subprocess.Popen([binary, "-Q", "-nw", *arguments, "--load", script],
                             stdin=slave, stdout=slave, stderr=slave,
                             preexec_fn=setup, close_fds=True, env=environment)
    transcript = bytearray()
    deadline = time.monotonic() + 120

    def pump():
        if select.select([master], [], [], 0.02)[0]:
            try:
                transcript.extend(os.read(master, 65536))
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
            if len(transcript) > 4 * 1024 * 1024:
                del transcript[:-4 * 1024 * 1024]

    try:
        while not (Path(marker).exists()
                   and not termios.tcgetattr(master)[3] & termios.ICANON):
            pump()
            if child.poll() is not None:
                raise AssertionError(f"editor exited before command input: {child.returncode}: {plain(transcript)[-3000:]}")
            if time.monotonic() >= deadline:
                raise AssertionError(f"editor did not enter its command loop: {plain(transcript)[-3000:]}")
        os.write(master, b"\x03\x12")  # C-c C-r, bound by the startup file.
        while child.poll() is None:
            pump()
            if time.monotonic() >= deadline:
                raise AssertionError(f"restart did not finish: {plain(transcript)[-3000:]}")
        assert child.returncode == 23, (child.returncode, plain(transcript)[-3000:])
        actual = termios.tcgetattr(master)
        restored = actual == original
        assert restored, (original, actual, plain(transcript)[-3000:])
        return {"status": child.returncode, "terminal_restored": restored}
    finally:
        if child.poll() is None:
            # The unreaped child is still the leader of this owned session.
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=5)
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    print(json.dumps(run(*sys.argv[1:4], sys.argv[4:])))
