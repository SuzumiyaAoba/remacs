;;; qa_sqlite_errors.el --- Native SQLite error data -*- lexical-binding: nil -*-
;; Run with qa_framework.el and SQLite-enabled GNU Emacs 31.1.

(defmacro q165-db (&rest body)
  `(let ((db (sqlite-open)))
     (unwind-protect (progn ,@body) (sqlite-close db))))

;; Return the actual condition data, without normalizing messages or codes.
;; For sets, also pin metadata and the retryable more-p state after one step.
(defun q165-call (db mode sql &optional values)
  (condition-case err
      (cond
       ((eq mode 'execute) (sqlite-execute db sql values))
       ((eq mode 'set)
        (let ((set (sqlite-select db sql values 'set)))
          (unwind-protect
              (list (sqlite-columns set)
                    (condition-case err (sqlite-next set) (error err))
                    (sqlite-more-p set))
            (sqlite-finalize set))))
       (t (sqlite-select db sql values mode)))
    (error err)))

;; Preparation is the only stage with the nested four-element payload.
;; SQLITE_ERROR is 1, not rusqlite ErrorCode::Unknown's discriminant.
(dolist (mode '(execute nil full set))
  (dolist (case '(("select * from nonexistent" "no such table: nonexistent")
                  ("select from" "near \"from\": syntax error")
                  ("select no_such_function()" "no such function: no_such_function")
                  ("select (" "incomplete input")
                  ("select :" "unrecognized token: \":\"")))
    (qa (q165-db (q165-call db mode (car case)))
        (list 'sqlite-error (list "SQL logic error" (cadr case) 1 1))))
  ;; SQLITE_ERROR_MISSING_COLLSEQ keeps its extended code, but the primary
  ;; prepare result is SQLITE_ERROR=1 even when SQLite returns 257 internally.
  (qa (q165-db (q165-call db mode "select 1 order by 1 collate no_such"))
      '(sqlite-error ("SQL logic error" "no such collation sequence: no_such" 1 257)))
  ;; A prepare error takes precedence over an excess binding error.
  (qa (q165-db (q165-call db mode "select from" [1 2]))
      '(sqlite-error ("SQL logic error" "near \"from\": syntax error" 1 1)))
  ;; Bind errors are strings in every mode, including a zero-slot statement.
  (dolist (case '(("select ?" (7 8)) ("select ?" [7 8])
                  ("select 1" [7]) ("select ?2" [1 2 3])
                  ("select :x, :x" [1 2])))
    (qa (q165-db (q165-call db mode (car case) (cadr case)))
        '(sqlite-error "column index out of range")))
  ;; A null prepared statement leaves the connection's native message alone.
  (dolist (sql '("" " \t\n" "-- comment" "/* comment */"))
    (qa (q165-db (q165-call db mode sql '(1)))
        '(sqlite-error "not an error"))
    (dolist (values '(nil []))
      (qa (q165-db (q165-call db mode sql values))
          (cond ((eq mode 'execute) '(sqlite-error "not an error"))
                ((eq mode 'set) '(nil (sqlite-error "not an error") t))
                ((eq mode 'full) '(nil))
                (t nil)))))
  ;; Step failures differ from prepare/bind failures: select returns rows so
  ;; far, execute signals only before its first row, and next stays retryable.
  (qa (q165-db (q165-call db mode "select abs(-9223372036854775808) as n"))
      (cond ((eq mode 'execute) '(sqlite-error "integer overflow"))
            ((eq mode 'set) '(("n") (sqlite-error "integer overflow") t))
            ((eq mode 'full) '(("n")))
            (t nil)))
  (qa (q165-db
        (q165-call db mode "select 1 as n union all select 2 union all select abs(-9223372036854775808)"))
      (cond ((eq mode 'set) '(("n") (1) t))
            ((eq mode 'full) '(("n") (1) (2)))
            (t '((1) (2)))))
  ;; Both scalar and RETURNING paths keep the same constraint message.
  (dolist (returning '(nil t))
    (qa (q165-db
          (sqlite-execute db "create table t(x unique)")
          (sqlite-execute db "insert into t values(1)")
          (q165-call db mode
                     (concat "insert into t values(1)"
                             (if returning " returning x" ""))))
        (cond ((eq mode 'execute) '(sqlite-error "UNIQUE constraint failed: t.x"))
              ((eq mode 'set)
               (list (when returning '("x"))
                     '(sqlite-error "UNIQUE constraint failed: t.x") t))
              ((eq mode 'full) (list (when returning '("x"))))
              (t nil)))))

;; Other extended constraint result codes must not leak into plain messages.
(dolist (case '(("create table t(x integer primary key); insert into t values(1)"
                 "insert into t values(1)" "UNIQUE constraint failed: t.x")
                ("create table t(x not null)" "insert into t values(NULL)"
                 "NOT NULL constraint failed: t.x")
                ("create table t(x check(x>0))" "insert into t values(-1)"
                 "CHECK constraint failed: x>0")
                ("PRAGMA foreign_keys=ON; create table p(x primary key); create table t(x references p)"
                 "insert into t values(1)" "FOREIGN KEY constraint failed")
                ("create table t(x); PRAGMA query_only=ON" "insert into t values(1)"
                 "attempt to write a readonly database")))
  (qa (q165-db
        (sqlite-execute-batch db (nth 0 case))
        (q165-call db 'execute (nth 1 case)))
      (list 'sqlite-error (nth 2 case))))

;; Failure leaves the connection usable and cannot execute a bad bind.
(qa (q165-db
      (sqlite-execute db "create table t(x unique)")
      (list (q165-call db 'execute "insert into t values(?)" [1 2])
            (sqlite-select db "select * from t")
            (sqlite-execute db "insert into t values(?)" [1])
            (q165-call db 'execute "insert into t values(1)")
            (sqlite-execute db "insert into t values(2)")
            (sqlite-select db "select * from t order by x")))
    '((sqlite-error "column index out of range") nil 1
      (sqlite-error "UNIQUE constraint failed: t.x") 1 ((1) (2))))

;; GNU returns changes after an empty result, and keeps all successful rows.
(qa (q165-db
      (sqlite-execute db "create table t(x)")
      (sqlite-execute db "insert into t values(1),(2)")
      (list (sqlite-execute db "select x from t where 0")
            (sqlite-execute db "select x from t order by x")
            (sqlite-execute db "update t set x=x+10 returning x")
            (sqlite-select db "select x from t order by x" nil 'full)))
    '(2 ((1) (2)) ((11) (12)) (("x") (11) (12))))

;; SQLITE_LOCKED at step time has a plain payload; only execute uses the
;; locked subtype. Select returns its accumulated result without a signal.
(dolist (mode '(execute nil full set))
  (qa (q165-db
        (sqlite-execute db "create table t(x)")
        (sqlite-execute db "insert into t values(1),(2)")
        (let ((set (sqlite-select db "select * from t" nil 'set)))
          (unwind-protect
              (progn (sqlite-next set) (q165-call db mode "drop table t"))
            (sqlite-finalize set))))
      (cond ((eq mode 'execute) '(sqlite-locked-error "database table is locked"))
            ((eq mode 'set) '(nil (sqlite-error "database table is locked") t))
            ((eq mode 'full) '(nil))
            (t nil))))

;; SQLITE_BUSY may arise at prepare or step, with different payload shapes.
(dolist (mode '(execute nil full set))
  (qa (let* ((path (make-temp-file "remacs-sqlite-error-" nil ".sqlite"))
             (db (sqlite-open path)) other)
        (unwind-protect
            (progn
              (sqlite-execute db "create table t(x)")
              (sqlite-execute db "begin exclusive")
              (setq other (sqlite-open path))
              (sqlite-pragma other "busy_timeout=0")
              (q165-call other mode "select * from t"))
          (sqlite-rollback db)
          (when other (sqlite-close other))
          (sqlite-close db)
          (delete-file path)))
      (list (if (eq mode 'execute) 'sqlite-locked-error 'sqlite-error)
            '("database is locked" "database is locked" 5 5)))
  (qa (let* ((path (make-temp-file "remacs-sqlite-error-" nil ".sqlite"))
             (db (sqlite-open path)) (other (sqlite-open path)))
        (unwind-protect
            (progn
              (sqlite-pragma db "busy_timeout=0")
              (sqlite-execute db "create table t(x)")
              (sqlite-execute db "insert into t values(1)")
              (sqlite-select db "select * from t")
              (sqlite-execute other "begin exclusive")
              (q165-call db mode "select * from t"))
          (sqlite-rollback other)
          (sqlite-close other)
          (sqlite-close db)
          (delete-file path)))
      (cond ((eq mode 'execute) '(sqlite-locked-error "database is locked"))
            ((eq mode 'set) '(("x") (sqlite-error "database is locked") t))
            ((eq mode 'full) '(("x")))
            (t nil))))

;; Preparation errors outside SQLITE_ERROR keep the actual native codes.
(dolist (mode '(execute nil full set))
  (qa (let ((path (make-temp-file "remacs-sqlite-notadb-" nil ".sqlite")) db)
        (unwind-protect
            (progn
              (write-region (make-string 256 ?x) nil path nil 'silent)
              (setq db (sqlite-open path))
              (q165-call db mode "select * from t"))
          (when db (sqlite-close db))
          (delete-file path)))
      '(sqlite-error ("file is not a database" "file is not a database" 26 26)))
  (qa (let* ((path (make-temp-file "remacs-sqlite-corrupt-" nil ".sqlite"))
             (db (sqlite-open path)))
        (unwind-protect
            (progn
              (sqlite-execute db "create table t(x)")
              (sqlite-execute db "PRAGMA writable_schema=ON")
              (sqlite-execute db "update sqlite_master set sql='malformed' where name='t'")
              (sqlite-close db)
              (setq db (sqlite-open path))
              (q165-call db mode "select * from t"))
          (sqlite-close db)
          (delete-file path)))
      '(sqlite-error ("database disk image is malformed" "malformed database schema (t)" 11 11)))
  ;; SQLITE_BUSY_SNAPSHOT is extended BUSY: execute still uses locked-error.
  (qa (let* ((path (make-temp-file "remacs-sqlite-snapshot-" nil ".sqlite"))
             (db (sqlite-open path)) (other (sqlite-open path)))
        (unwind-protect
            (progn
              (sqlite-execute db "PRAGMA journal_mode=WAL")
              (sqlite-execute db "create table t(x)")
              (sqlite-execute db "insert into t values(1)")
              (sqlite-transaction db)
              (sqlite-select db "select * from t")
              (sqlite-execute other "insert into t values(2)")
              (q165-call db mode "insert into t values(3)"))
          (sqlite-rollback db)
          (sqlite-close other)
          (sqlite-close db)
          (delete-file path)))
      (cond ((eq mode 'execute) '(sqlite-locked-error "database is locked"))
            ((eq mode 'set) '(nil (sqlite-error "database is locked") t))
            ((eq mode 'full) '(nil))
            (t nil))))

;; Quiet operations retain their nil failure contracts and error hierarchy.
(qa (q165-db
      (list (sqlite-execute-batch db "select from")
            (sqlite-commit db) (sqlite-rollback db)
            (sqlite-load-extension db "/nonexistent/remacs-qa-sqlite-errors/rot13.so")
            (q165-call db 'execute "select load_extension('rot13')")
            (sqlite-select db "select 1")))
    '(nil nil nil nil (sqlite-error "not authorized") ((1))))
(qa (get 'sqlite-error 'error-conditions) '(sqlite-error error))
(qa (get 'sqlite-locked-error 'error-conditions)
    '(sqlite-locked-error sqlite-error error))

(princ (qa--done))
(terpri)
