;;; -*- lexical-binding: t -*-
;; #3379: the complete source must load past the state-cache function and
;; reach its final provide, using the real cc-defs/cc-vars dependencies.
(require 'cc-engine)

(qa (featurep 'cc-engine) t)
(qa (fboundp 'c-state-cache-init) t)
(qa (c-delq-from-dotted-list 'a (copy-tree '(a b a . tail))) '(b . tail))

(princ (qa--done))
