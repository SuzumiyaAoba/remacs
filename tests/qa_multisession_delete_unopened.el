;;; qa_multisession_delete_unopened.el --- SQLite delete initialization regression -*- lexical-binding: t -*-

(require 'multisession)

(defun qa-multisession-delete-unopened-result ()
  (let* ((directory (make-temp-file "multisession-delete-unopened-" t))
         (user-init-file "/tmp/init.el")
         (multisession-directory directory)
         (multisession--db nil)
         (object (make-multisession :key "missing" :package "probe"
                                    :storage 'sqlite)))
    (unwind-protect
        (condition-case error-data
            (progn
              (multisession-delete object)
              (if multisession--db 'ok 'not-open))
          (error (list (car error-data) (error-message-string error-data))))
      (when multisession--db
        (sqlite-close multisession--db))
      (delete-directory directory t))))

(when (sqlite-available-p)
  (if qa--remacs
      (qa (qa-multisession-delete-unopened-result) 'ok)
    ;; GNU 31.1 currently calls sqlite-execute with a nil handle here.
    (qa-xf (qa-multisession-delete-unopened-result) 'ok
           "Remacs initializes SQLite before deleting a missing multisession key")))

(princ (qa--done))
