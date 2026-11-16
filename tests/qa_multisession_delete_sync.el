;;; qa_multisession_delete_sync.el --- synchronized deletion regression -*- lexical-binding: t -*-

(require 'multisession)

(defun qa-multisession-delete-sync-result (storage)
  (let* ((directory (make-temp-file "multisession-delete-probe-" t))
         (user-init-file "/tmp/init.el")
         (multisession-directory directory)
         (multisession-storage storage)
         (multisession--db nil)
         (a (make-multisession :key "k" :package "probe"
                               :initial-value "initial"
                               :synchronized t :storage storage))
         (b (make-multisession :key "k" :package "probe"
                               :initial-value "initial"
                               :synchronized t :storage storage))
         (c (make-multisession :key "k" :package "probe"
                               :initial-value "initial"
                               :synchronized t :storage storage)))
    (unwind-protect
        (progn
          (setf (multisession-value a) "stored")
          (let ((before (multisession-value b)))
            (multisession-delete a)
            (let ((after (list (multisession-value a)
                               (multisession-value b)
                               (multisession-value c))))
              (setf (multisession-value c) "new")
              (list before after (multisession-value b)))))
      (when multisession--db
        (sqlite-close multisession--db))
      (delete-directory directory t))))

(defun qa-multisession-delete-old-sqlite-schema-result ()
  (let* ((directory (make-temp-file "multisession-old-schema-" t))
         (user-init-file "/tmp/init.el")
         (multisession-directory directory)
         (multisession-storage 'sqlite)
         (multisession--db nil)
         (file (expand-file-name "sqlite/multisession.sqlite" directory)))
    (unwind-protect
        (progn
          (make-directory (file-name-directory file) t)
          (let ((db (sqlite-open file)))
            (sqlite-execute
             db
             "create table multisession (package text not null, key text not null, sequence number not null default 1, value text not null)")
            (sqlite-execute
             db
             "create unique index multisession_idx on multisession (package, key)")
            (sqlite-execute
             db
             "insert into multisession(package, key, sequence, value) values('probe', 'legacy', 5, '\"stored\"')")
            (sqlite-close db))
          (let ((object (make-multisession :key "legacy" :package "probe"
                                           :initial-value "initial"
                                           :synchronized t :storage 'sqlite)))
            (let ((before (multisession-value object)))
              (multisession-delete object)
              (list before
                    (multisession-value object)
                    (and (member "deleted"
                                 (mapcar #'cadr
                                         (sqlite-select
                                          multisession--db
                                          "pragma table_info(multisession)")))
                         t)))))
      (when multisession--db
        (sqlite-close multisession--db))
      (delete-directory directory t))))

(let ((expected '("stored" ("initial" "initial" "initial") "new")))
  (if qa--remacs
       (qa (qa-multisession-delete-sync-result 'files) expected)
     (qa-xf (qa-multisession-delete-sync-result 'files) expected
            "Remacs synchronizes deletion tombstones for the files backend"))
  (when (sqlite-available-p)
    (if qa--remacs
        (qa (qa-multisession-delete-sync-result 'sqlite) expected)
      (qa-xf (qa-multisession-delete-sync-result 'sqlite) expected
             "Remacs synchronizes deletion tombstones for SQLite"))))

(when (sqlite-available-p)
  (if qa--remacs
      (qa (qa-multisession-delete-old-sqlite-schema-result)
          '("stored" "initial" t))
    (qa-xf (qa-multisession-delete-old-sqlite-schema-result)
           '("stored" "initial" t)
           "Remacs migrates old SQLite tables for deletion tombstones")))

(princ (qa--done))
