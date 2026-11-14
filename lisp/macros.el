;;; macros.el --- keyboard macro conveniences -*- lexical-binding: t -*-

;; Subset of GNU's macros.el: query during macro execution, naming,
;; and inserting macros as Lisp.

(defun kbd-macro-query (flag)
  "Query user during kbd macro execution.
With prefix argument FLAG, enter recursive edit, reading keyboard
commands even within a kbd macro.  Without prefix argument, ask
whether to continue running the macro.

Your options are: \\<query-replace-map>

\\[act]\tFinish this iteration normally and continue with the next.
\\[skip]\tSkip the rest of this iteration, and start the next.
\\[exit]\tStop the macro entirely right now.
\\[recenter]\tRedisplay the screen, then ask again.
\\[edit]\tEnter recursive edit; ask again when you exit from that."
  (interactive "P")
  (or executing-kbd-macro
      defining-kbd-macro
      (user-error "Not defining or executing kbd macro"))
  (if flag
      (let (executing-kbd-macro defining-kbd-macro)
        (recursive-edit))
    (if (not executing-kbd-macro)
        nil
      (let ((loop t)
            (msg (substitute-command-keys
                  "Proceed with macro?\\<query-replace-map>\
 (\\[act], \\[skip], \\[exit], \\[recenter], \\[edit]) ")))
        (while loop
          (let ((key (let ((executing-kbd-macro nil)
                           (defining-kbd-macro nil))
                       (message "%s" msg)
                       (read-event)))
                def)
            (setq key (vector key))
            (setq def (lookup-key query-replace-map key))
            (cond ((eq def 'act)
                   (setq loop nil))
                  ((eq def 'skip)
                   (setq loop nil)
                   (setq executing-kbd-macro ""))
                  ((eq def 'exit)
                   (setq loop nil)
                   (setq executing-kbd-macro t))
                  ((eq def 'recenter)
                   (recenter nil))
                  ((eq def 'edit)
                   (let (executing-kbd-macro defining-kbd-macro)
                     (recursive-edit)))
                  ((eq def 'quit)
                   (setq quit-flag t))
                  (t
                   (or (eq def 'help)
                       (ding))
                   (with-output-to-temp-buffer "*Help*"
                     (princ
                      (substitute-command-keys
                       "Specify how to proceed with keyboard macro execution.
Possibilities: \\<query-replace-map>
\\[act]\tFinish this iteration normally and continue with the next.
\\[skip]\tSkip the rest of this iteration, and start the next.
\\[exit]\tStop the macro entirely right now.
\\[recenter]\tRedisplay the screen, then ask again.
\\[edit]\tEnter recursive edit; ask again when you exit from that."))
                     (with-current-buffer standard-output
                       (help-mode)))))))))))

(defun insert-kbd-macro (macroname &optional keys)
  "Insert in buffer the Lisp definition of kbd macro MACRONAME.
Optional second argument KEYS means also record its global key bindings."
  (interactive "SName of keyboard macro to insert: \nP")
  (let ((def (symbol-function macroname)))
    (or (or (stringp def) (vectorp def))
        (error "Keyboard macro %s is not defined" macroname))
    (princ "(fset '" (current-buffer))
    (princ macroname (current-buffer))
    (princ "\n   " (current-buffer))
    (prin1 def (current-buffer))
    (princ ")\n" (current-buffer))
    (when keys
      (let ((keys (or (where-is-internal def '(keymap))
                      (where-is-internal macroname '(keymap)))))
        (dolist (key keys)
          (princ "(keymap-global-set " (current-buffer))
          (prin1 (key-description key) (current-buffer))
          (princ " #'" (current-buffer))
          (prin1 macroname (current-buffer))
          (princ ")\n" (current-buffer)))))))

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
