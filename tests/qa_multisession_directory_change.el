;;; qa_multisession_directory_change.el --- SQLite directory switch regression -*- lexical-binding: t -*-

(require 'multisession)

(defun qa-multisession-directory-change-result ()
  (let* ((directory-a (make-temp-file "multisession-dir-a-" t))
         (directory-b (make-temp-file "multisession-dir-b-" t))
         (user-init-file "/tmp/init.el")
         (multisession-directory directory-a)
         (multisession-storage 'sqlite)
         (multisession--db nil)
         (object (make-multisession :key "k" :package "probe"
                                    :initial-value "initial"
                                    :synchronized t :storage 'sqlite))
         (file-a (expand-file-name "sqlite/multisession.sqlite" directory-a))
         (file-b (expand-file-name "sqlite/multisession.sqlite" directory-b)))
    (unwind-protect
        (progn
          (setf (multisession-value object) "directory-a")
          (let ((db-a multisession--db))
            (setq multisession-directory directory-b)
            (let ((value (multisession-value object)))
              (list (eq db-a multisession--db)
                    (file-exists-p file-a)
                    (file-exists-p file-b)
                    value))))
      (when multisession--db
        (sqlite-close multisession--db))
      (delete-directory directory-a t)
      (delete-directory directory-b t))))

(when (sqlite-available-p)
  (if qa--remacs
      (qa (qa-multisession-directory-change-result) '(nil t t "initial"))
    ;; GNU 31.1 keeps using the original connection after a directory change.
    (qa-xf (qa-multisession-directory-change-result) '(nil t t "initial")
           "Remacs reconnects to a changed multisession-directory")))

(princ (qa--done))
