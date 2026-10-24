#!/usr/bin/env python3
"""Exercise verified TLS against a disposable local HTTPS server."""
import argparse
import json
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading

ROOT = Path(__file__).resolve().parents[1]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--gnu", required=True)
    ap.add_argument("--output-dir", default="tests/audit/current/full-suite")
    args = ap.parse_args()
    out = ROOT / args.output_dir
    results, server = [], []
    with tempfile.TemporaryDirectory(prefix="remacs-tls-audit-") as td:
        td = Path(td)
        cert, key = td / "cert.pem", td / "key.pem"
        p = subprocess.run(["/usr/bin/openssl", "req", "-x509", "-newkey", "rsa:2048",
                            "-nodes", "-keyout", str(key), "-out", str(cert), "-days", "1",
                            "-subj", "/CN=localhost", "-addext",
                            "subjectAltName=DNS:localhost,IP:127.0.0.1"],
                           capture_output=True, timeout=20)
        assert p.returncode == 0, p.stderr
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen()
        listener.settimeout(.5)
        port = listener.getsockname()[1]
        stop = threading.Event()

        def serve():
            while not stop.is_set():
                try:
                    raw, _ = listener.accept()
                except socket.timeout:
                    continue
                try:
                    raw.settimeout(10)
                    with context.wrap_socket(raw, server_side=True) as conn:
                        request = conn.recv(10000)
                        conn.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ntls-ok\n")
                        server.append(dict(tls=conn.version(), requested_get=request.startswith(b"GET ")))
                except Exception as error:
                    server.append(dict(error=type(error).__name__, detail=str(error)))

        thread = threading.Thread(target=serve, daemon=True)
        thread.start()
        try:
            form = r'''(progn (require 'network-stream) (require 'gnutls)
 (let ((gnutls-trustfiles (list CERT)) (gnutls-verify-error t)
       (network-security-level 'high) (buffer (generate-new-buffer " *audit-tls*")) p)
  (unwind-protect
   (progn
    (setq p (open-gnutls-stream "audit-tls" buffer "127.0.0.1" PORT))
    (if (not p)
        (prin1 (list 'no-process (with-current-buffer "*Messages*" (buffer-string))))
      (set-process-query-on-exit-flag p nil)
      (process-send-string p "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
      (let ((n 0)) (while (and (process-live-p p) (< n 200))
        (accept-process-output p 0.02) (setq n (1+ n))))
      (prin1 (with-current-buffer buffer
        (list (and (string-match-p "tls-ok" (buffer-string)) t) (process-type p))))))
   (when p (ignore-errors (delete-process p))) (kill-buffer buffer))))'''
            form = form.replace("CERT", json.dumps(str(cert))).replace("PORT", str(port))
            for side, binary in (("gnu", args.gnu), ("remacs", str(ROOT / "target/debug/remacs"))):
                try:
                    p = subprocess.run([binary, "-Q", "--batch", "--eval", form],
                                       capture_output=True, text=True, timeout=45)
                    (out / f"tls-local.{side}.out").write_text(p.stdout)
                    (out / f"tls-local.{side}.err").write_text(p.stderr)
                    result = dict(side=side, returncode=p.returncode, stdout=p.stdout,
                                  passed=p.returncode == 0 and p.stdout == "(t network)")
                except subprocess.TimeoutExpired:
                    result = dict(side=side, timed_out=True, passed=False)
                results.append(result)
                print(side, result, flush=True)
        finally:
            stop.set()
            thread.join(timeout=2)
            listener.close()
    (out / "tls-local-results.json").write_text(json.dumps(dict(
        results=results, server=server,
        trust="Temporary test CA explicitly trusted; verification enabled; private key removed."),
        ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
