;;; qa_rmail_from_line.el --- Rmail From-line edit regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/rmailedit.el"
       (file-name-directory load-file-name)))

(defconst qa-rmail-from-line-message
  (concat "From sender@example.test Fri Jan 1 00:00:00 2026\n"
          "Subject: test\n\n"
          "From body@example.test\n"
          "body\n"))

(qa
 (with-temp-buffer
   (insert qa-rmail-from-line-message)
   (let ((rmail-old-text qa-rmail-from-line-message))
     (rmail-edit--text-changed-p rmail-old-text)))
 nil)

(qa
 (with-temp-buffer
   (insert qa-rmail-from-line-message)
   (let ((rmail-old-text qa-rmail-from-line-message))
     (cl-letf (((symbol-function 'rmail-summary-exists)
                (lambda () nil))
               ((symbol-function 'rmail-cease-edit--finish)
                (lambda (_abort changed-before-escaping)
                  (goto-char (point-min))
                  (search-forward "\n\n")
                  (re-search-forward "^From ")
                  (beginning-of-line)
                  (insert ">")
                  (list changed-before-escaping
                        (rmail-edit--text-changed-p rmail-old-text)))))
       (rmail-cease-edit))))
 '(nil t))

(qa
 (with-temp-buffer
   (insert qa-rmail-from-line-message "user edit\n")
   (rmail-edit--text-changed-p qa-rmail-from-line-message))
 t)

(princ (qa--done))
