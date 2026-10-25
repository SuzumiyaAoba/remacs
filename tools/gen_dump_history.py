#!/usr/bin/env python3
"""Capture GNU's dumped library provenance without retaining installation paths.

Usage: GNU_EMACS=/path/to/emacs-31.1 python3 tools/gen_dump_history.py [--check]
The checked-in manifest is data for reconstructing the equivalent dump;
normal builds and startup do not invoke GNU Emacs.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "crates/runtime/src/lisp/dump_history.json"
EXPRESSION = r'''
(let ((dump-history-generator--names nil)
      (dump-history-generator--history (copy-tree load-history))
      (coding-system-for-write 'utf-8-unix))
  (mapatoms
   (lambda (symbol)
     (unless (string-prefix-p "dump-history-generator--" (symbol-name symbol))
       (push (symbol-name symbol) dump-history-generator--names))))
  ;; json-serialize returns UTF-8 octets; print Unicode, not raw byte chars.
  (princ
   (decode-coding-string
    (json-serialize
     (list :version emacs-version :root lisp-directory
           :symbols (vconcat (sort dump-history-generator--names #'string-lessp))
           :libraries
           (vconcat
            (mapcar
             (lambda (entry)
               (list :file (car entry)
                     :entries (vconcat (mapcar #'prin1-to-string (cdr entry)))))
             dump-history-generator--history))))
    'utf-8)))
'''



def generate(emacs: str) -> str:
    output = subprocess.run(
        [emacs, "-Q", "--batch", "--eval", EXPRESSION],
        text=True, encoding="utf-8", capture_output=True, check=True,
    )
    reference = json.loads(output.stdout)
    if reference["version"] != "31.1":
        raise ValueError(f"expected GNU Emacs 31.1, got {reference['version']}")
    root = Path(reference["root"])
    libraries = []
    for library in reference["libraries"]:
        path = Path(library["file"])
        relative = path.relative_to(root)
        if path.suffix not in (".el", ".elc"):
            raise ValueError(f"unexpected dump library: {path}")
        parts = list(relative.with_suffix("").parts)
        # GNU first-level Lisp directories are separate load-path entries.
        # Deeper namespaces (semantic/ctxt, quail/emoji, ...) retain theirs.
        key = "/".join(parts[1:] if len(parts) > 1 else parts)
        libraries.append({"key": key, "entries": library["entries"]})
    data = {
        "generated_by": "tools/gen_dump_history.py",
        "format": 1,
        "reference_version": reference["version"],
        "known_symbols": reference["symbols"],
        "libraries": libraries,
    }
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emacs", default=os.environ.get("GNU_EMACS", os.environ.get("EMACS", "emacs")))
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generated = generate(args.emacs)
    if args.check:
        if args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"{args.output} is stale; regenerate with GNU Emacs 31.1")
    else:
        args.output.write_text(generated, encoding="utf-8")
    data = json.loads(generated)
    print(f"{len(data['libraries'])} libraries; {len(data['known_symbols'])} reference symbols")


if __name__ == "__main__":
    main()
