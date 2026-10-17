;;; qa_sqlite_lazy_sets.el --- Deferred SQLite set execution -*- lexical-binding: nil -*-
;; Run with qa_framework.el and SQLite-enabled GNU Emacs 31.1.

(defmacro q164-db (&rest body)
  `(let ((db (sqlite-open)) set)
     (unwind-protect
         (progn ,@body)
       (when set (condition-case nil (sqlite-finalize set) (sqlite-error nil)))
       (sqlite-close db))))

(defun q164-rows (db)
  (sqlite-select db "select n from data order by n"))

;; The exact audit example: constructing a set does not insert anything.
(qa (q164-db
      (sqlite-execute db "create table data(n)")
      (setq set (sqlite-select db "insert into data values(7) returning n" nil 'set))
      (q164-rows db))
    nil)

;; Metadata and more-p never step; each next advances once, and EOF prevents
;; SQLite's automatic reset from executing the statement a second time.
(qa (q164-db
      (sqlite-execute db "create table data(n)")
      (setq set (sqlite-select db "insert into data values(7),(8) returning n" nil 'set))
      (list (sqlite-columns set) (sqlite-more-p set) (q164-rows db)
            (sqlite-next set) (q164-rows db) (sqlite-more-p set)
            (sqlite-next set) (sqlite-more-p set)
            (sqlite-next set) (sqlite-more-p set)
            (sqlite-next set) (q164-rows db)))
    '(("n") t nil (7) ((7) (8)) t (8) t nil nil nil ((7) (8))))

;; INSERT, UPDATE, and DELETE are lazy with and without RETURNING. Finalizing
;; an unstepped set cancels it, while finalizing after a step keeps its effects.
(dolist (case '(("insert into data values(3)" ((1) (2) (3)) (3))
                ("update data set n=n+10" ((11) (12)) (11))
                ("delete from data where n=1" ((2)) (1))))
  (dolist (returning '(nil t))
    (dolist (advance '(nil t))
      (qa (q164-db
            (sqlite-execute db "create table data(n)")
            (sqlite-execute db "insert into data values(1),(2)")
            (setq set (sqlite-select db
                        (concat (car case) (if returning " returning n" "")) nil 'set))
            (let ((before (q164-rows db))
                  (row (when advance (sqlite-next set))))
              (sqlite-finalize set)
              (list before row (q164-rows db))))
          (list '((1) (2)) (when (and advance returning) (nth 2 case))
                (if advance (nth 1 case) '((1) (2))))))))

(qa (q164-db
      (setq set (sqlite-select db "create table data(n)" nil 'set))
      (list (sqlite-columns set) (sqlite-more-p set)
            (sqlite-select db "select name from sqlite_master where name='data'")
            (sqlite-next set) (sqlite-more-p set)
            (sqlite-select db "select name from sqlite_master where name='data'")
            (sqlite-next set)))
    '(nil t nil nil nil (("data")) nil))

;; The ordinary and full result types must remain eager.
(dolist (mode '(nil full))
  (qa (q164-db
        (sqlite-execute db "create table data(n)")
        (list (sqlite-select db "insert into data values(7) returning n" nil mode)
              (q164-rows db)))
      (list (if mode '(("n") (7)) '((7))) '((7)))))

;; Read queries see changes made before their first step, and their cursor
;; stays live between steps. Empty sets start with more-p true too.
(qa (q164-db
      (sqlite-execute db "create table data(n)")
      (setq set (sqlite-select db "select n from data" nil 'set))
      (sqlite-execute db "insert into data values(1),(2)")
      (let ((first (sqlite-next set)))
        (sqlite-execute db "insert into data values(3)")
        (list first (sqlite-next set) (sqlite-next set)
              (sqlite-next set) (sqlite-more-p set))))
    '((1) (2) (3) nil nil))
(qa (q164-db
      (setq set (sqlite-select db "select 1 as n where 0" nil 'set))
      (list (sqlite-columns set) (sqlite-more-p set) (sqlite-next set)
            (sqlite-more-p set) (sqlite-next set) (sqlite-columns set)))
    '(("n") t nil nil nil ("n")))

;; Validation remains eager, but constraints and expression errors are raised
;; by next. A failed step must not mark EOF or discard the prepared statement.
(dolist (sql '("select from" "select * from missing"))
  (qa-err (q164-db (sqlite-select db sql nil 'set)) 'sqlite-error))
(qa-err (q164-db (sqlite-select db "select ?" [1 2] 'set)) 'sqlite-error)
(qa (q164-db
      (setq set (sqlite-select db "select abs(-9223372036854775808) as n" nil 'set))
      (list (sqlite-columns set) (sqlite-more-p set)
            (condition-case err (sqlite-next set) (error err))
            (sqlite-more-p set)
            (condition-case err (sqlite-next set) (error err))))
    '(("n") t (sqlite-error "integer overflow") t (sqlite-error "integer overflow")))
(qa (q164-db
      (setq set (sqlite-select db
                  "select case n when 2 then abs(-9223372036854775808) else n end from (select 1 n union all select 2)"
                  nil 'set))
      (list (sqlite-next set)
            (condition-case err (sqlite-next set) (error err))
            (sqlite-more-p set) (sqlite-next set)))
    '((1) (sqlite-error "integer overflow") t (1)))
(qa (q164-db
      (sqlite-execute db "create table data(n unique)")
      (sqlite-execute db "insert into data values(1)")
      (setq set (sqlite-select db "insert into data values(1) returning n" nil 'set))
      (let ((error-result (condition-case err (sqlite-next set) (error err)))
            (more (sqlite-more-p set)))
        (sqlite-execute db "delete from data")
        (list error-result more (sqlite-next set) (sqlite-next set)
              (sqlite-more-p set) (q164-rows db))))
    '((sqlite-error "UNIQUE constraint failed: data.n") t (1) nil nil ((1))))

;; Column metadata belongs to the actual statement, which SQLite can
;; automatically reprepare after schema changes before the first step.
(qa (q164-db
      (sqlite-execute db "create table data(n)")
      (sqlite-execute db "insert into data values(1)")
      (setq set (sqlite-select db "select * from data" nil 'set))
      (let ((initial (sqlite-columns set)))
        (sqlite-execute db "alter table data add column extra default 2")
        (list initial (sqlite-columns set) (sqlite-next set) (sqlite-columns set))))
    '(("n") ("n") (1 2) ("n" "extra")))
(qa (q164-db
      (sqlite-execute db "create table data(n)")
      (setq set (sqlite-select db "select * from data" nil 'set))
      (sqlite-execute db "drop table data")
      (let ((error-result (condition-case err (sqlite-next set) (error err))))
        (sqlite-execute db "create table data(a,b)")
        (sqlite-execute db "insert into data values(3,4)")
        (list error-result (sqlite-more-p set) (sqlite-next set) (sqlite-columns set))))
    '((sqlite-error "no such table: data") t (3 4) ("a" "b")))

;; A live statement keeps its connection alive after the database handle is
;; closed. Cover never-stepped, active, and exhausted sets, and two live sets.
(dolist (steps '(0 1 3))
  (qa (q164-db
        (setq set (sqlite-select db "select 1 n union all select 2" nil 'set))
        (dotimes (_ steps) (sqlite-next set))
        (sqlite-close db)
        (list (condition-case err (sqlite-select db "select 3") (error err))
              (sqlite-columns set) (sqlite-more-p set) (sqlite-next set)
              (sqlite-finalize set) (sqlite-close db)))
      (list '(sqlite-error "Database closed") '("n") (< steps 3)
            (cond ((= steps 0) '(1)) ((= steps 1) '(2))) t t)))
(qa (q164-db
      (setq set (sqlite-select db "select 1" nil 'set))
      (let ((second (sqlite-select db "select 2" nil 'set)))
        (unwind-protect
            (progn (sqlite-close db) (sqlite-finalize set) (sqlite-next second))
          (sqlite-finalize second))))
    '(2))
(dolist (fn '(sqlite-next sqlite-more-p sqlite-columns sqlite-finalize))
  (qa-err (q164-db
            (setq set (sqlite-select db "select 1" nil 'set))
            (sqlite-finalize set)
            (funcall fn set))
          'sqlite-error))

;; A live RETURNING statement retains transaction state until exhausted or
;; finalized. Eagerly collecting its rows would allow COMMIT too soon.
(dolist (finish '(exhaust finalize))
  (qa (q164-db
        (sqlite-execute db "create table data(n)")
        (sqlite-transaction db)
        (setq set (sqlite-select db "insert into data values(1) returning n" nil 'set))
        (let ((row (sqlite-next set)) (early-commit (sqlite-commit db)))
          (if (eq finish 'exhaust) (sqlite-next set) (sqlite-finalize set))
          (list row early-commit (sqlite-commit db) (q164-rows db))))
      '((1) nil t ((1)))))

;; Bindings, types, and omitted values retain the established behavior.
(dolist (values '(nil [] (7) [7] (7 "text") [7 "text"]))
  (qa (q164-db
        (setq set (sqlite-select db "select ? as a, ? as b" values 'set))
        (list (sqlite-columns set) (sqlite-next set) (sqlite-next set)))
      (list '("a" "b") (if (or (null values) (equal values [])) '(nil nil)
                           (if (= (length values) 1) '(7 nil) '(7 "text"))) nil)))
(qa (q164-db
      (let ((values (vector 1 2)))
        (setq set (sqlite-select db "select ?, ?" values 'set))
        (aset values 0 99)
        (sqlite-next set)))
    '(1 2))
(qa (q164-db
      (let ((values (list 3 4)))
        (setq set (sqlite-select db "select ?, ?" values 'set))
        (setcar values 99)
        (sqlite-next set)))
    '(3 4))
(qa (q164-db
      (setq set (sqlite-select db "select 1, 1.5, 'text', NULL, x'', x'0080ff'" nil 'set))
      (let ((row (sqlite-next set)))
        (list (nth 0 row) (nth 1 row) (nth 2 row) (nth 3 row)
              (string-to-list (nth 4 row)) (multibyte-string-p (nth 4 row))
              (string-to-list (nth 5 row)) (multibyte-string-p (nth 5 row)))))
    '(1 1.5 "text" nil nil nil (0 128 255) nil))

;; Preserve SQLite's SQL byte handling: embedded NUL terminates the input,
;; and a set represents the first prepared statement only.
(dolist (sql (list (concat "select 1" (string 0) "select from")
                  "select 1; select from"))
  (qa (q164-db
        (setq set (sqlite-select db sql nil 'set))
        (list (sqlite-columns set) (sqlite-next set)))
      '(("1") (1))))
(qa-err (q164-db
          (sqlite-select db (concat "select 'a" (string 0) "b'") nil 'set))
        'sqlite-error)

;; Empty SQL still prepares a handle; SQLite reports misuse only on next.
(dolist (sql (list "" "-- comment" "/* comment */" (concat (string 0) "select 1")))
  (qa (q164-db
        (setq set (sqlite-select db sql nil 'set))
        (list (sqlite-columns set) (sqlite-more-p set)
              (condition-case err (sqlite-next set) (error err))
              (sqlite-more-p set)))
      '(nil t (sqlite-error "not an error") t)))

;; Native prepare semantics remain in force: some PRAGMAs apply at prepare
;; time even though no sqlite3_step has happened yet.
(qa (q164-db
      (setq set (sqlite-select db "pragma foreign_keys=on" nil 'set))
      (sqlite-select db "pragma foreign_keys"))
    '((1)))

;; Exercise native locks across two connections. A prepared read has no read
;; lock until stepped; DONE/finalize releases it even after its DB was closed.
(defmacro q164-file-db (&rest body)
  `(let* ((path (make-temp-file "remacs-sqlite-lazy-" nil ".sqlite"))
          (db (sqlite-open path)) (other (sqlite-open path)) set)
     (unwind-protect
         (progn
           (sqlite-pragma db "busy_timeout=0")
           (sqlite-pragma other "busy_timeout=0")
           (sqlite-execute db "create table data(n)")
           (sqlite-execute db "insert into data values(1),(2)")
           ,@body)
       (when set (condition-case nil (sqlite-finalize set) (sqlite-error nil)))
       (sqlite-close db)
       (sqlite-close other)
       (delete-file path))))
(qa (q164-file-db
      (setq set (sqlite-select db "select * from data" nil 'set))
      (list (sqlite-execute other "insert into data values(3)")
            (sqlite-next set)
            (condition-case err (sqlite-execute other "insert into data values(4)")
              (error (car err)))
            (sqlite-next set) (sqlite-next set) (sqlite-next set)
            (sqlite-execute other "insert into data values(5)")
            (sqlite-next set)))
    '(1 (1) sqlite-locked-error (2) (3) nil 1 nil))
(qa (q164-file-db
      (setq set (sqlite-select db "select * from data" nil 'set))
      (sqlite-execute other "begin exclusive")
      (let ((error-result (condition-case err (sqlite-next set) (error err)))
            (more (sqlite-more-p set)))
        (sqlite-rollback other)
        (list error-result more (sqlite-next set))))
    '((sqlite-error "database is locked") t (1)))
(qa (q164-file-db
      (setq set (sqlite-select db "select * from data" nil 'set))
      (sqlite-next set)
      (sqlite-close db)
      (let ((error-symbol (condition-case err
                              (sqlite-execute other "insert into data values(3)")
                            (error (car err)))))
        (sqlite-finalize set)
        (list error-symbol (sqlite-execute other "insert into data values(4)"))))
    '(sqlite-locked-error 1))

(princ (qa--done))
(terpri)
