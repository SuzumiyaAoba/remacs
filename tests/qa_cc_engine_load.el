;;; -*- lexical-binding: t -*-
;; #1964/#3379: requiring cc-mode must load the full cc-engine source and
;; reach its final provide, using the real cc-defs/cc-vars dependencies.
(require 'cc-mode)

(qa (featurep 'cc-engine) t)
(qa (fboundp 'c-state-cache-init) t)
(qa (c-delq-from-dotted-list 'a (copy-tree '(a b a . tail))) '(b . tail))

(princ (qa--done))
