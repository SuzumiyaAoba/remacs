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

;; Keymap for the C-x C-k prefix (autoloaded from global-map).
(fset 'kmacro-keymap
  '(keymap (120 . kmacro-to-register) (110 . kmacro-name-last-macro) (98 . kmacro-bind-to-key) (32 . kmacro-step-edit-macro) (108 . kmacro-edit-lossage) (101 . edit-kbd-macro) (13 . kmacro-edit-macro) (5 . kmacro-edit-macro-repeat) (17 keymap (62 . kmacro-quit-counter-greater) (60 . kmacro-quit-counter-less) (61 . kmacro-quit-counter-equal)) (18 keymap (97 keymap (62 . kmacro-reg-add-counter-greater) (60 . kmacro-reg-add-counter-less) (61 . kmacro-reg-add-counter-equal)) (115 . kmacro-reg-save-counter) (108 . kmacro-reg-load-counter)) (1 . kmacro-add-counter) (9 . kmacro-insert-counter) (3 . kmacro-set-counter) (6 . kmacro-set-format) (12 . kmacro-call-ring-2nd-repeat) (20 . kmacro-swap-ring) (4 . kmacro-delete-ring-head) (22 . kmacro-view-macro-repeat) (16 . kmacro-cycle-ring-previous) (14 . kmacro-cycle-ring-next) (100 . kmacro-redisplay) (113 . kbd-macro-query) (114 . apply-macro-to-region-lines) (11 . kmacro-end-or-call-macro-repeat) (19 . kmacro-start-macro) (115 . kmacro-start-macro)))

(provide 'kmacro)
