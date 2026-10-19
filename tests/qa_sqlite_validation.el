;;; qa_sqlite_validation.el --- SQLite handle validation -*- lexical-binding: nil -*-
;; Expected conditions verified against SQLite-enabled GNU Emacs 31.1.

(defconst q166-db-calls
  '((sqlite-close) (sqlite-execute "select 1") (sqlite-select "select 1")
    (sqlite-execute-batch "select 1") (sqlite-transaction) (sqlite-commit)
    (sqlite-rollback) (sqlite-pragma "user_version")
    (sqlite-load-extension "/nonexistent/remacs-qa-sqlite/rot13.so")))
(defconst q166-set-calls
  '((sqlite-next) (sqlite-more-p) (sqlite-columns) (sqlite-finalize)))
(defconst q166-bad-args
  '((sqlite-execute nil) (sqlite-execute "select ?" 1)
    (sqlite-select nil) (sqlite-select "select ?" 1)
    (sqlite-execute-batch nil) (sqlite-pragma nil)
    (sqlite-load-extension nil) (sqlite-load-extension "not-allowed.so")))
(defun q166-call (object call)
  (condition-case err
      (apply (car call) object (cdr call))
    (error err)))

;; Exact issue reproduction, including the rejected object in signal data.
(qa (let ((db (sqlite-open)))
      (unwind-protect
          (let ((fake (record 'sqlite 'db 999999)))
            (list (sqlitep fake) (q166-call fake '(sqlite-close))))
        (sqlite-close db)))
    '(nil (wrong-type-argument sqlitep #s(sqlite db 999999))))

;; Layout, tags, ID type/range, and guessed IDs cannot create SQLite objects.
;; Include vector/list lookalikes and uninterned tag/kind symbols.
(dolist (object
         (list nil t 42 "sqlite" '(sqlite db 1) [sqlite db 1]
               (record 'sqlite) (record 'sqlite 'db)
               (record 'sqlite 'db 999999) (record 'sqlite 'set 999999)
               (record 'sqlite 'db 0) (record 'sqlite 'db -1)
               (record 'sqlite 'db 18446744073709551616)
               (record 'sqlite 'db 1.0) (record 'sqlite 'db "1")
               (record 'sqlite 'db nil) (record 'sqlite 'other 1)
               (record 'sqlite "db" 1) (record 'other 'db 1)
               (record 'sqlite 'db 1 'extra)
               (record (make-symbol "sqlite") 'db 1)
               (record 'sqlite (make-symbol "db") 1)))
  (qa (sqlitep object) nil)
  (dolist (call (append q166-db-calls q166-set-calls q166-bad-args))
    (qa (q166-call object call)
        (list 'wrong-type-argument 'sqlitep object))))

;; In Remacs the record slots expose the native registry ID. GNU's opaque
;; pseudovectors do not expose it, but every corresponding plain record must
;; still be rejected. A copied/reconstructed record is never the original.
(let* ((db (sqlite-open))
       (set (sqlite-select db "select 7 as n union all select 8" nil 'set))
       (db-id (if (recordp db) (aref db 2) 1))
       (set-id (if (recordp set) (aref set 2) 2))
       (fakes (list (record 'sqlite 'db db-id)
                    (record 'sqlite 'set set-id)
                    (record 'sqlite 'db set-id)
                    (record 'sqlite 'set db-id)
                    (if (recordp db) (copy-sequence db)
                      (record 'sqlite 'db db-id))
                    (if (recordp set) (copy-sequence set)
                      (record 'sqlite 'set set-id)))))
  (unwind-protect
      (progn
        (dolist (fake fakes)
          (qa (sqlitep fake) nil)
          (dolist (call (append q166-db-calls q166-set-calls))
            (qa (q166-call fake call)
                (list 'wrong-type-argument 'sqlitep fake))))
        ;; Rejection must not close the DB, consume rows, or finalize the set.
        (qa (q166-call db '(sqlite-select "select 9")) '((9)))
        (qa (q166-call set '(sqlite-columns)) '("n"))
        (qa (q166-call set '(sqlite-more-p)) t)
        (qa (q166-call set '(sqlite-next)) '(7))
        (qa (q166-call set '(sqlite-next)) '(8))
        (qa (q166-call set '(sqlite-next)) nil)
        (qa (q166-call set '(sqlite-more-p)) nil))
    (q166-call set '(sqlite-finalize))
    (sqlite-close db))
  ;; Retired IDs and copies are still invalid after their originals close.
  (dolist (fake fakes)
    (qa (sqlitep fake) nil)
    (qa (q166-call fake '(sqlite-close))
        (list 'wrong-type-argument 'sqlitep fake))
    (qa (q166-call fake '(sqlite-finalize))
        (list 'wrong-type-argument 'sqlitep fake))))

;; Genuine database identity and aliases survive close. All non-close DB
;; operations check liveness before any SQL/value/module argument validation.
(let* ((db (sqlite-open)) (alias (car (list db))))
  (qa (list (eq db alias) (sqlitep db) (sqlitep alias)) '(t t t))
  (qa (sqlite-select alias "select 11") '((11)))
  (dolist (call q166-set-calls)
    (qa (q166-call db call) '(sqlite-error "Invalid set object")))
  (qa (sqlite-close alias) t)
  (qa (sqlite-close db) t)
  (qa (sqlitep db) t)
  (dolist (call (append (cdr q166-db-calls) q166-bad-args))
    (qa (q166-call db call) '(sqlite-error "Database closed")))
  (dolist (call q166-set-calls)
    (qa (q166-call db call) '(sqlite-error "Invalid set object"))))

;; Genuine sets use kind errors for DB operations and closed-statement errors
;; after finalize. Closing a finalized set is GNU's idempotent-close exception.
(let* ((db (sqlite-open)) (set (sqlite-select db "select 13" nil 'set))
       (alias (car (list set))))
  (unwind-protect
      (progn
        (qa (list (eq set alias) (sqlitep set) (sqlitep alias)) '(t t t))
        (dolist (call (append q166-db-calls q166-bad-args))
          (qa (q166-call set call) '(sqlite-error "Invalid database object")))
        (qa (sqlite-next alias) '(13))
        (qa (sqlite-finalize alias) t)
        (qa (sqlitep set) t)
        (qa (sqlite-close set) t)
        (dolist (call (append (cdr q166-db-calls) q166-bad-args))
          (qa (q166-call set call) '(sqlite-error "Invalid database object")))
        (dolist (call q166-set-calls)
          (qa (q166-call set call) '(sqlite-error "Statement closed"))))
    (q166-call set '(sqlite-finalize))
    (sqlite-close db)))

;; Closing the DB object does not close outstanding statement objects.
(qa (let* ((db (sqlite-open))
           (set (sqlite-select db "select 17 as n union all select 19" nil 'set)))
      (unwind-protect
          (progn
            (sqlite-close db)
            (list (sqlitep db) (sqlitep set) (sqlite-columns set)
                  (sqlite-next set) (sqlite-next set) (sqlite-next set)
                  (sqlite-more-p set) (sqlite-finalize set) (sqlitep set)))
        (q166-call set '(sqlite-finalize))
        (sqlite-close db)))
    '(t t ("n") (17) (19) nil nil t t))

(princ (qa--done))
(terpri)
