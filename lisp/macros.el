;;; macros.el --- keyboard macro conveniences -*- lexical-binding: t -*-

;; Subset of GNU's macros.el: query during macro execution, naming,
;; and inserting macros as Lisp.

(defun kbd-macro-query (flag)
  "Query user during kbd macro execution.
With prefix argument FLAG, enter recursive edit, reading keyboard
commands even within a kbd macro.  Without FLAG, ask whether to
continue running the macro."
  (interactive "P")
  ;; Non-interactive/no-window-system: never query, always continue.
  nil)

(defun insert-kbd-macro (macroname)
  "Insert in buffer the Lisp definition of kbd macro MACRONAME."
  (interactive "SName of keyboard macro to insert: ")
  (let ((def (symbol-function macroname)))
    (or (or (stringp def) (vectorp def))
        (error "Keyboard macro %s is not defined" macroname))
    (princ "(fset '" (current-buffer))
    (princ macroname (current-buffer))
    (princ "\n   " (current-buffer))
    (prin1 def (current-buffer))
    (princ ")\n" (current-buffer))))

(provide 'macros)
