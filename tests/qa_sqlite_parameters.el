;;; qa_sqlite_parameters.el --- Omitted SQLite values -*- lexical-binding: nil -*-
;; Run with qa_framework.el and SQLite-enabled GNU Emacs 31.1.

(defun q163-query (mode sql values)
  (let ((db (sqlite-open)) set)
    (unwind-protect
        (cond
         ((eq mode 'execute) (sqlite-execute db sql values))
         ((eq mode 'set)
          (setq set (sqlite-select db sql values 'set))
          (let ((columns (sqlite-columns set))
                (initial-more (sqlite-more-p set)) rows row)
            (while (setq row (sqlite-next set)) (push row rows))
            (list columns initial-more (nreverse rows) (sqlite-more-p set))))
         (t (sqlite-select db sql values mode)))
      (when set (sqlite-finalize set))
      (sqlite-close db))))

(defun q163-result (mode columns rows)
  (cond ((eq mode 'full) (cons columns rows))
        ((eq mode 'set) (list columns t rows nil))
        (t rows)))

;; Truly omitted VALUES, explicit nil, an empty list, and an empty vector
;; all leave every parameter NULL.  Lists and vectors bind by slot index.
(dolist (fn '(sqlite-select sqlite-execute))
  (qa (let ((db (sqlite-open)))
        (unwind-protect (funcall fn db "select ?, ?") (sqlite-close db)))
      '((nil nil))))

(dolist (mode '(nil full set execute))
  (dolist (case '((nil (nil nil)) ([] (nil nil))
                  ((7) (7 nil)) ([7] (7 nil))
                  ((nil) (nil nil)) ([nil] (nil nil))
                  ((7 "x") (7 "x")) ([7 nil] (7 nil))
                  ((t 1.5) (1 1.5))))
    (qa (q163-query mode "select ? as a, ? as b" (car case))
        (q163-result mode '("a" "b") (list (cadr case)))))

  ;; Numbered holes count as slots even when no SQL expression uses them.
  ;; Repeated names and repeated indices share one slot, not one per use.
  (dolist (case '(("select ?3 as a, ?1 as b, ?3 as c" nil (nil nil nil))
                  ("select ?3 as a, ?1 as b, ?3 as c" (7) (nil 7 nil))
                  ("select ?3 as a, ?1 as b, ?3 as c" [7 8] (nil 7 nil))
                  ("select ?3 as a, ?1 as b, ?3 as c" [7 8 9] (9 7 9))
                  ("select :x as a, :x as b, :y as c" nil (nil nil nil))
                  ("select :x as a, :x as b, :y as c" [7] (7 7 nil))
                  ("select :x as a, :x as b, :y as c" (7 8) (7 7 8))
                  ("select @x as a, $y as b, @x as c" (7) (7 nil 7))
                  ("select ?2 as a, :x as b, ? as c" (7 8 9) (8 9 nil))
                  ("select ?1 as a, ?1 as b, ?1 as c" [7] (7 7 7))))
    (qa (q163-query mode (nth 0 case) (nth 1 case))
        (q163-result mode '("a" "b" "c") (list (nth 2 case)))))

  ;; NULL works in expressions and filters; empty result sets still have
  ;; column metadata and the same lazy more-p transition as full sets.
  (qa (q163-query mode "select coalesce(?, 42) as a, ? is null as b" nil)
      (q163-result mode '("a" "b") '((42 1))))
  (unless (eq mode 'execute)
    (qa (q163-query mode "select ? as a where ?" '(7))
        (q163-result mode '("a") nil)))
  (qa (q163-query mode "select ? as a union all select ?" '(7))
      (q163-result mode '("a") '((7) (nil))))
  ;; SQL without parameters and question marks in literals/comments must
  ;; not be mistaken for bind slots.
  (qa (q163-query mode "select '?' as a /* ?99 :unused */" [])
      (q163-result mode '("a") '(("?"))))

  ;; Padding must never truncate excess values or suppress prepare errors.
  (dolist (case '(("select ?" (1 2)) ("select ?" [1 2])
                  ("select 1" [1]) ("select ?2" [1 2 3])
                  ("select :x, :x" [1 2])))
    (qa-err (q163-query mode (car case) (cadr case)) 'sqlite-error))
  (qa-err (q163-query mode "select from" nil) 'sqlite-error))

;; Both execute branches (affected rows and RETURNING rows) bind missing
;; values identically, and a failed bind cannot mutate the database.
(dolist (returning '(nil t))
  (dolist (case '((nil (nil nil)) ([] (nil nil))
                  ((7) (7 nil)) ([7] (7 nil))
                  ((7 "x") (7 "x"))))
    (qa (let ((db (sqlite-open)))
          (unwind-protect
              (progn
                (sqlite-execute db "create table data (a, b)")
                (let ((result (sqlite-execute db
                               (concat "insert into data values (?, ?)"
                                       (if returning " returning a, b" ""))
                               (car case))))
                  (list result (sqlite-select db "select a, b from data"))))
            (sqlite-close db)))
        (list (if returning (list (cadr case)) 1) (list (cadr case))))))

(qa (let ((db (sqlite-open)))
      (unwind-protect
          (progn
            (sqlite-execute db "create table data (a, b)")
            (list (sqlite-execute db "insert into data values (?, ?)")
                  (sqlite-execute db "update data set a = ?, b = ?" [7])
                  (sqlite-select db "select a, b from data")
                  (sqlite-execute db "delete from data where b is ?")
                  (sqlite-select db "select a, b from data")))
        (sqlite-close db)))
    '(1 1 ((7 nil)) 1 nil))

(dolist (returning '(nil t))
  (qa (let ((db (sqlite-open)))
        (unwind-protect
            (progn
              (sqlite-execute db "create table data (a, b)")
              (list (condition-case err
                        (sqlite-execute db
                          (concat "insert into data values (?, ?)"
                                  (if returning " returning a, b" "")) [1 2 3])
                      (error (car err)))
                    (sqlite-select db "select a, b from data")))
          (sqlite-close db)))
      '(sqlite-error nil)))

;; sqlite-select also accepts statements that mutate the database.  The
;; same padding applies with or without RETURNING and with every result
;; type, including a set stepped to completion.
(dolist (mode '(nil full set))
  (dolist (returning '(nil t))
    (dolist (values '(nil [7]))
      (qa (let ((db (sqlite-open)) set)
            (unwind-protect
                (progn
                  (sqlite-execute db "create table data (a, b)")
                  (let ((result (sqlite-select db
                                  (concat "insert into data values (?, ?)"
                                          (if returning " returning a, b" ""))
                                  values mode)))
                    (when (eq mode 'set)
                      (setq set result)
                      (while (sqlite-next set)))
                    (sqlite-select db "select a, b from data")))
              (when set (sqlite-finalize set))
              (sqlite-close db)))
          (list (if values '(7 nil) '(nil nil)))))))

;; Repeated prepares on the same connection must not retain earlier binds.
(qa (let ((db (sqlite-open)))
      (unwind-protect
          (list (sqlite-select db "select ?, ?" [7 8])
                (sqlite-select db "select ?, ?" [9])
                (sqlite-select db "select ?, ?")
                (sqlite-select db "select ?, ?" []))
        (sqlite-close db)))
    '(((7 8)) ((9 nil)) ((nil nil)) ((nil nil))))

;; Omitted values are still subject to SQL constraints, and normal API
;; validation remains in force.
(qa-err (let ((db (sqlite-open)))
          (unwind-protect
              (progn
                (sqlite-execute db "create table required (a not null)")
                (sqlite-execute db "insert into required values (?)"))
            (sqlite-close db)))
        'sqlite-error)
(dolist (fn '(sqlite-select sqlite-execute))
  (qa-err (funcall fn 42 "select ?") 'wrong-type-argument)
  (qa-err (let ((db (sqlite-open)))
            (unwind-protect (funcall fn db 42) (sqlite-close db)))
          'wrong-type-argument)
  (qa-err (let ((db (sqlite-open)))
            (sqlite-close db) (funcall fn db "select ?"))
          'sqlite-error))

(princ (qa--done))
(terpri)
