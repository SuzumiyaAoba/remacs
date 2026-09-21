;;; kmacro.el --- keyboard macro ring -*- lexical-binding: t -*-

;; Subset of GNU's kmacro.el: start/end commands and ring execution.

(defun kmacro-start-macro (arg)
  "Record subsequent keyboard input, defining a keyboard macro.
The macro is ended with \\[kmacro-end-macro] or \\[kmacro-end-or-call-macro]."
  (interactive "P")
  (start-kbd-macro arg nil))

(defun kmacro-end-macro (arg)
  "Finish defining a keyboard macro.
The macro was started with \\[kmacro-start-macro]."
  (interactive "P")
  (end-kbd-macro arg))

(defun kmacro-start-macro-or-insert-counter (arg)
  "Start defining a keyboard macro, or insert the counter value."
  (interactive "P")
  (start-kbd-macro arg nil))

(defun kmacro-end-or-call-macro (arg &optional no-repeat)
  "End the current macro or call the last macro."
  (interactive "P")
  (if defining-kbd-macro
      (end-kbd-macro arg)
    (call-last-kbd-macro arg)))

(defun kmacro-end-or-call-macro-repeat (arg)
  "Like `kmacro-end-or-call-macro' but repeats immediately."
  (interactive "P")
  (kmacro-end-or-call-macro arg))

(defun kmacro-name-last-macro (symbol)
  "Assign a name to the last keyboard macro defined."
  (interactive "SName for last kbd macro: ")
  (or last-kbd-macro
      (error "No keyboard macro defined"))
  (fset symbol last-kbd-macro)
  symbol)

(provide 'kmacro)
