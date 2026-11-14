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

;;;###autoload
(defun apply-macro-to-region-lines (top bottom &optional macro)
  "Apply last keyboard macro to all lines in the region.
For each line that begins in the region, move to the beginning of
the line, and run the last keyboard macro.

When called from lisp, this function takes two arguments TOP and
BOTTOM, describing the current region.  TOP must be before BOTTOM.
The optional third argument MACRO specifies a keyboard macro to
execute."
  (interactive "r")
  (or macro
      (progn
        (if (null last-kbd-macro)
            (user-error "No keyboard macro has been defined"))
        (setq macro last-kbd-macro)))
  (save-excursion
    (let ((end-marker (copy-marker bottom))
          next-line-marker)
      (goto-char top)
      (if (not (bolp))
          (forward-line 1))
      (setq next-line-marker (point-marker))
      (while (< next-line-marker end-marker)
        (goto-char next-line-marker)
        (save-excursion
          (forward-line 1)
          (set-marker next-line-marker (point)))
        (save-excursion
          (let ((mark-active nil))
            (execute-kbd-macro macro))))
      (set-marker end-marker nil)
      (set-marker next-line-marker nil))))

(provide 'macros)
