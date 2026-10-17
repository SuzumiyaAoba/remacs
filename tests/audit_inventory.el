;;; audit_inventory.el --- startup function inventory, without calling targets -*- lexical-binding: t; -*-
;; This lists names only.  Presence here is NOT execution coverage.
(let (names)
  (mapatoms (lambda (symbol) (when (fboundp symbol) (push (symbol-name symbol) names))))
  (princ (json-serialize (vconcat (sort names #'string<)))))
