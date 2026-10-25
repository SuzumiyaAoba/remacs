#!/usr/bin/env python3
"""Snapshot GNU 31.1 forwarded variables without changing their bindings.

Usage: python3 tools/gen_forwarded_variables.py --emacs /path/to/emacs
The committed table makes protection independent of a runtime DOC file.
"""

import argparse
import json
from pathlib import Path
import subprocess


EXPRESSION = r"""
(progn
  (unless (equal emacs-version "31.1") (error "Expected GNU Emacs 31.1"))
  (defvar remacs--probe-base nil)
  (let (candidates forwarded)
    (mapatoms
     (lambda (variable)
       (when (and (boundp variable)
                  (integerp (get variable 'variable-documentation))
                  (null (symbol-file variable 'defvar)))
         (push variable candidates))))
    (dolist (variable candidates)
      ;; A dynamic let guarantees that non-builtins also reject the alias.
      ;; Keep the base bound so the probe cannot propagate a value to it.
      (let ((result
             (condition-case error
                 (eval (list 'let
                             (list (list variable (list 'quote (symbol-value variable))))
                             (list 'defvaralias (list 'quote variable) ''remacs--probe-base)) nil)
               (error error))))
        (when (and (eq (car-safe result) 'error)
                   (string-prefix-p "Cannot make a built-in variable an alias: "
                                    (cadr result)))
          (push (vector (symbol-name variable)
                        (if (local-variable-if-set-p variable) "B" "G"))
                forwarded))))
    (princ (json-serialize (vconcat forwarded)))))
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emacs", default="emacs")
    args = parser.parse_args()
    result = subprocess.run(
        [args.emacs, "-Q", "--batch", "--eval", EXPRESSION],
        check=True, capture_output=True, text=True,
    )
    entries = sorted(json.loads(result.stdout))
    if not entries or any(any(c.isspace() for c in name) for name, _ in entries):
        raise ValueError("Invalid forwarded variable snapshot")
    destination = Path(__file__).resolve().parents[1] / "crates/runtime/src/lisp/forwarded_variables.txt"
    contents = "".join(f"{kind} {name}\n" for name, kind in entries)
    if not destination.exists() or destination.read_text() != contents:
        destination.write_text(contents)
    print(f"{len(entries)} GNU forwarded variables -> {destination}")


if __name__ == "__main__":
    main()
