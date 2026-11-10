;;; qa_rmail_continue_no_draft.el --- Rmail continue guard regression -*- lexical-binding:t -*-

(defvar mail-user-agent nil)
(defvar qa-rmail-start-mail-calls nil)

(defun qa-rmail-start-mail-stub (&rest args)
  (push args qa-rmail-start-mail-calls)
  'started)

(defun qa-rmail-load-continue-source ()
  "Evaluate `rmail-continue' from rmail.el source."
  (let ((file (locate-library "rmail"))
        found)
    (unless file
      (error "Can't locate rmail source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'rmail-continue))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Missing rmail-continue definition"))
    'loaded))

(qa (qa-rmail-load-continue-source) 'loaded)

(qa
 (let ((mail-user-agent 'sendmail-user-agent)
       (qa-rmail-start-mail-calls nil)
       (original-start-mail (symbol-function 'rmail-start-mail)))
   (unwind-protect
       (progn
         (fset 'rmail-start-mail #'qa-rmail-start-mail-stub)
         (list (condition-case nil
                   (rmail-continue)
                 (user-error 'no-draft))
               qa-rmail-start-mail-calls))
     (fset 'rmail-start-mail original-start-mail)))
 '(no-draft nil))

(princ (qa--done))
