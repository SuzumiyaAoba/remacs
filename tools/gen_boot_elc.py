#!/usr/bin/env python3
"""AOT byte-compile the boot-time libraries with GNU Emacs.

remacs boots by loading the same set of files GNU Emacs dumps into
temacs (see `boot_lib' in src/lisp/eval.rs).  GNU loads those libraries
as byte-compiled .elc at dump time; we interpret the .el sources, which
costs ~900 ms per startup.  This script byte-compiles each boot library
with a reference GNU Emacs and emits the resulting .elc blobs under
lisp-elc/ plus a Rust table src/lisp/embedded_elc.rs that the loader
consults before the embedded .el source.

Requirements:

    EMACS=<gnu-emacs-31.1 binary> python3 tools/gen_boot_elc.py

Compilation runs inside one Emacs process.  `remacs-compat' (a
remacs-only shim file) is pre-provided so our cl-lib's `(require
'remacs-compat)' resolves without loading the shim itself.  Files GNU
marks `no-byte-compile' (charprop.el) are skipped automatically.

Regenerate whenever a booted lisp/*.el file changes — like
tools/gen_embedded.py for the EMBEDDED_LISP table.
"""

import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LISP = os.path.join(ROOT, "lisp")
OUT = os.path.join(ROOT, "lisp-elc")
GEN = os.path.join(ROOT, "src", "lisp", "embedded_elc.rs")

# Every library `boot_lib'/`load_library' pulls in during Interp::new,
# in load order.  charprop.el is `no-byte-compile: t' like GNU's;
# remacs-compat.el has no GNU counterpart.
SKIP = {
    "charprop", "remacs-compat", "cl-loaddefs",
    # Compiled twins whose load-time forms regress QA / boot: seq/
    # subr-x call cl-* helpers before cl-lib is bound, nadvice wraps
    # already-installed cells, pcase's with-eval-after-load hook
    # double-fires, loaddefs replays stale autoload cells, cl-lib/
    # cl-generic redefine mid-boot machinery, tex-mode touches
    # display state.  Boot keeps the interpreted .el for these.
    "cl-lib", "cl-generic", "pcase", "seq", "subr-x", "nadvice",
    "tex-mode", "loaddefs",
}
# Note: most boot libraries compile to a working twin.  The SKIP set
# above keeps eight exceptions on the interpreted path; see its
# comment for why each regressed QA / boot parity.
# Files byte-compiled but also needing the .el read-time path kept
# (nothing at the moment — kept for future exceptions).
BOOT_LIBS = [
    "byte-run", "subr", "seq", "env", "jka-cmpr-hook", "tabulated-list",
    "subr-x", "tool-bar", "widget", "version", "vc-hooks", "uniquify",
    "tooltip", "float-sup", "map-ynp", "debug-early", "oclosure",
    "cl-macs", "cl-lib", "cl-generic", "nadvice", "loaddefs", "tab-bar",
    "icons", "image", "buff-menu", "electric", "format", "composite",
    "iso-transl", "mule-util", "mule", "mule-conf", "mule-cmds",
    "case-table", "characters", "epa-hook", "paren", "rmc",
    # Language/charset tables (loadup.el:219-247).
    "chinese", "cyrillic", "indian", "sinhala", "english", "ethiopic",
    "european", "czech", "slovak", "romanian", "greek", "hebrew",
    "cp51932", "eucjp-ms", "japanese", "korean", "lao", "tai-viet",
    "thai", "tibetan", "vietnamese", "misc-lang", "utf-8-lang",
    "georgian", "khmer", "burmese", "cham", "philippine", "indonesian",
    # Rolled/secondary dumped libs.
    "abbrev", "cconv", "cus-face", "ediff-hook", "eldoc", "mouse",
    "rect", "send-to", "map", "prog-mode", "regexp-opt", "register",
    "replace", "scroll-bar", "text-mode", "timer", "newcomment",
    "ns-win", "common-win", "fontset", "mwheel", "pcase", "help-macro",
    "inline",
]

ELISP = r"""
;; -*- lexical-binding: t -*-
(setq byte-compile-dynamic-docstrings nil
      byte-compile-dynamic nil
      byte-compile-warnings nil
      byte-compile-verbose nil)
;; Our cl-lib `(require 'remacs-compat)' — satisfy at compile time; the
;; real file only exists for the remacs runtime.
(provide 'remacs-compat)
(let ((missing ()))
  (dolist (f command-line-args-left)
    (condition-case e
        (byte-compile-file f)
      (error (princ (format "ERR %s: %s\n" f e))))
    (unless (file-exists-p (concat (file-name-sans-extension f) ".elc"))
      (push (file-name-nondirectory f) missing)))
  (when missing
    (princ (format "MISSING: %s\n" missing))))
"""


def main() -> int:
    emacs = os.environ.get("EMACS")
    if not emacs:
        for cand in [
            "/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1/bin/emacs",
            "/usr/bin/emacs",
        ]:
            if os.path.exists(cand):
                emacs = cand
                break
        else:
            emacs = "emacs"
    os.makedirs(OUT, exist_ok=True)
    script = os.path.join(OUT, ".gen.el")
    with open(script, "w") as f:
        f.write(ELISP)

    sources = []
    for lib in BOOT_LIBS:
        if lib in SKIP:
            continue
        src = os.path.join(LISP, f"{lib}.el")
        if not os.path.exists(src):
            print(f"skip {lib}: no lisp/{lib}.el", file=sys.stderr)
            continue
        sources.append(src)
    # Compile into OUT: byte-compile-file writes next to the source, so
    # stage copies in OUT instead of polluting lisp/.
    staged = []
    for src in sources:
        dst = os.path.join(OUT, os.path.basename(src))
        with open(src, "rb") as r, open(dst, "wb") as w:
            w.write(r.read())
        staged.append(dst)

    r = subprocess.run(
        [emacs, "-Q", "--batch", "-l", script] + staged,
        capture_output=True,
        text=True,
    )
    sys.stderr.write(r.stderr)
    sys.stdout.write(r.stdout)
    if r.returncode != 0:
        print(f"emacs exited {r.returncode}", file=sys.stderr)
        return 1

    emitted = {}
    for src in staged:
        stem = os.path.splitext(os.path.basename(src))[0]
        elc = os.path.join(OUT, f"{stem}.elc")
        if os.path.exists(elc):
            emitted[stem] = elc
        else:
            print(f"no .elc: {stem}", file=sys.stderr)

    lines = [
        "// Generated by tools/gen_boot_elc.py — byte-compiled boot libraries.",
        "// Each blob is a GNU Emacs 31.1 `.elc' (text) file; the loader",
        "// prefers it over the embedded `.el' source like GNU's openp.",
        "",
        "pub(crate) static EMBEDDED_ELC: &[(&str, &str)] = &[",
    ]
    for stem in BOOT_LIBS:
        if stem in emitted:
            lines.append(f'    ("{stem}", include_str!("../../lisp-elc/{stem}.elc")),')
    lines.append("];\n")
    with open(GEN, "w") as f:
        f.write("\n".join(lines))
    print(f"embedded_elc.rs: {len(emitted)} libraries")
    # Clean staged .el copies; keep the .elc blobs.
    for src in staged:
        os.remove(src)
    os.remove(script)
    return 0


if __name__ == "__main__":
    sys.exit(main())
