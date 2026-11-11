;;; -*- lexical-binding: t -*-
;; #1964/#3379: requiring cc-mode must load the full cc-engine source and
;; reach its final provide, using the real cc-defs/cc-vars dependencies.
(require 'cc-mode)

(qa (featurep 'cc-engine) t)
(qa (fboundp 'c-state-cache-init) t)
(qa (c-delq-from-dotted-list 'a (copy-tree '(a b a . tail))) '(b . tail))

;; #2069: Every top-level form in the shipped source remains readable.
(qa
 (with-temp-buffer
   (insert-file-contents (locate-library "cc-engine"))
   (goto-char (point-min))
   (while (progn
            (skip-chars-forward " \t\n\r")
            (not (eobp)))
     (read (current-buffer)))
   t)
 t)

(princ (qa--done))
