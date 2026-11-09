;;; -*- lexical-binding: nil; -*-
;; A before advice on call-interactively must not hide the interactive caller.
(load-file (expand-file-name "../lisp/nadvice.el"
                             (file-name-directory load-file-name)))
(defun qa1777-command ()
  (interactive)
  (called-interactively-p 'interactive))
(let ((noninteractive nil)
      (executing-kbd-macro nil))
  (advice-add 'call-interactively :before #'ignore)
  (unwind-protect
      (progn
        (qa (not (null (advice-member-p #'ignore 'call-interactively))) t)
        (qa (call-interactively #'qa1777-command) t)
        ;; The command loop uses command-execute's internal dispatcher.
        (qa (command-execute #'qa1777-command) t))
    (advice-remove 'call-interactively #'ignore)))
(princ (qa--done))
