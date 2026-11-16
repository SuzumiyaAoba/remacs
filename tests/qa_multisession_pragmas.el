;;; qa_multisession_pragmas.el --- SQLite initialization regression -*- lexical-binding: t -*-

(require 'multisession)

(defun qa-multisession-pragma-results ()
  (let ((directory (make-temp-file "multisession-probe-" t))
        (multisession--db nil))
    (let ((multisession-directory directory))
      (unwind-protect
          (progn
            (multisession--ensure-db)
            (list (sqlite-select multisession--db "pragma journal_mode")
                  (sqlite-select multisession--db "pragma synchronous")))
        (when multisession--db
          (sqlite-close multisession--db))
        (delete-directory directory t)))))

(when (sqlite-available-p)
  (if qa--remacs
      (qa (qa-multisession-pragma-results) '((("wal")) ((1))))
    ;; GNU 31.1 currently executes both PRAGMAs inside the transaction.
    (qa-xf (qa-multisession-pragma-results) '((("wal")) ((1)))
           "Remacs enables WAL and NORMAL synchronous mode")))

(princ (qa--done))
