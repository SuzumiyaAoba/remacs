;;; audit_sqlite_lifecycle_more.el --- SQLite path and GC lifecycle -*- lexical-binding: t; -*-
(load (expand-file-name "tests/audit_helpers.el") nil t)

(defun audit-sqlite-forget-transaction (file close)
  (let ((db (sqlite-open file)))
    (sqlite-execute db "create table data(n)")
    (sqlite-execute db "begin immediate")
    (sqlite-execute db "insert into data values(1)")
    (when close (sqlite-close db))
    nil))

(defun audit-sqlite-gc-lock (close)
  (let ((file (make-temp-file "remacs-sqlite-gc-" nil ".sqlite")) other)
    (unwind-protect
        (progn
          (audit-sqlite-forget-transaction file close)
          (garbage-collect)
          (garbage-collect)
          (setq other (sqlite-open file))
          (sqlite-pragma other "busy_timeout=0")
          (condition-case err
              (sqlite-execute other "insert into data values(2)")
            (error (car err))))
      (when other (sqlite-close other))
      (delete-file file))))

(audit-check "sqlite:gc-open-transaction" '(audit-sqlite-gc-lock nil))
(audit-check "sqlite:gc-explicit-close-control" '(audit-sqlite-gc-lock t))

(defun audit-sqlite-forget-set (file finalize)
  (let* ((db (sqlite-open file))
         (statement (sqlite-select db "select * from data" nil 'set)))
    (sqlite-next statement)
    (when finalize (sqlite-finalize statement))
    (sqlite-close db)
    nil))

(defun audit-sqlite-gc-set-lock (finalize)
  (let* ((file (make-temp-file "remacs-sqlite-gc-set-" nil ".sqlite"))
         (other (sqlite-open file)))
    (unwind-protect
        (progn
          (sqlite-execute other "create table data(n)")
          (sqlite-execute other "insert into data values(1),(2)")
          (sqlite-pragma other "busy_timeout=0")
          (audit-sqlite-forget-set file finalize)
          (garbage-collect)
          (garbage-collect)
          (condition-case err
              (sqlite-execute other "insert into data values(3)")
            (error (car err))))
      (sqlite-close other)
      (delete-file file))))

(audit-check "sqlite:gc-active-set" '(audit-sqlite-gc-set-lock nil))
(audit-check "sqlite:gc-explicit-finalize-control" '(audit-sqlite-gc-set-lock t))

(audit-check "sqlite:open-default-directory"
 '(let* ((dir (file-name-as-directory (make-temp-file "remacs-sqlite-dir-" t)))
         (original-directory default-directory)
         (base (concat (make-temp-name "remacs-sqlite-relative-") ".sqlite"))
         (wanted (expand-file-name base dir))
         (unwanted (expand-file-name base original-directory)) db)
    (unwind-protect
        (let ((default-directory dir))
          (setq db (sqlite-open base))
          (list (sqlitep db) (file-exists-p wanted) (file-exists-p unwanted)))
      (when db (sqlite-close db))
      (when (file-exists-p unwanted) (delete-file unwanted))
      (delete-directory dir t))))

(princ "AUDIT-DONE\n")
