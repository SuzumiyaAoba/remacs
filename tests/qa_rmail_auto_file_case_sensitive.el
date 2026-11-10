;;; qa_rmail_auto_file_case_sensitive.el --- Rmail matching case regression -*- lexical-binding:t -*-

(require 'cl-lib)

(defvar rmail-total-messages)
(defvar rmail-current-message)
(defvar rmail-file-name)
(defvar rmail-automatic-folder-directives)

(defun qa-rmail-load-auto-file-function ()
  "Evaluate the current checkout's `rmail-auto-file'."
  (let ((file (expand-file-name
               "../lisp/rmail.el"
               (file-name-directory load-file-name))))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (unless (search-forward "(defun rmail-auto-file" nil t)
        (error "Can't find rmail-auto-file"))
      (goto-char (match-beginning 0))
      (eval (read (current-buffer)) t))
    'loaded))

(defun qa-rmail-auto-file-folder (subject)
  "Return the destination selected by `rmail-auto-file' for SUBJECT."
  (let ((rmail-total-messages 1)
        (rmail-current-message 1)
        (rmail-file-name "/tmp/RMAIL")
        (rmail-automatic-folder-directives
         '(("Work" "subject" "^IMPORTANT")))
        (case-fold-search t)
        filed)
    (with-temp-buffer
      (setq buffer-file-name "/tmp/RMAIL")
      (cl-letf (((symbol-function 'rmail-message-attr-p)
                 (lambda (&rest _arguments) nil))
                ((symbol-function 'mail-fetch-field)
                 (lambda (field)
                   (when (equal (downcase field) "subject") subject)))
                ((symbol-function 'rmail-output)
                 (lambda (folder _count) (setq filed folder))))
        (rmail-auto-file))
      filed)))

(qa (qa-rmail-load-auto-file-function) 'loaded)

;; Ambient case-fold-search is t in Rmail, but the directive is documented
;; to match case-sensitively.
(qa (list (qa-rmail-auto-file-folder "Important")
          (qa-rmail-auto-file-folder "IMPORTANT"))
    '(nil "Work"))

(princ (qa--done))
