;;; -*- lexical-binding: t -*-
;; #3380: expand cc-defs through its real byte compiler and dependencies.
;; No custom-declare-face or other dependency stubs.
(require 'cc-defs)

(qa (featurep 'cc-defs) t)
(qa c-version "5.35.2")
(qa (cc-bytecomp-compiling-or-loading) 'loading)
(qa (cc-eval-when-compile (+ 1 2)) 3)
(qa (with-temp-buffer
      (insert "first\nsecond")
      (goto-char (point-max))
      (c-point 'bol))
    7)

(princ (qa--done))
