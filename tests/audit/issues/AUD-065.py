import json, os, pathlib, select, subprocess, sys, tempfile, time
from pathlib import Path
ROOT = Path.cwd()
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


with tempfile.TemporaryDirectory(prefix='remacs-bind-repro-') as tmp:
    print(inspect(Path(sys.argv[1]).resolve(), 'repro', Path(tmp)))
