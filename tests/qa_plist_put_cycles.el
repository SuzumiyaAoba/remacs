;;; -*- lexical-binding: nil; -*-
;; Put uses strict pair validation and live value cells (#508-#510).
(dolist (row '((1 0 1) (2 0 1) (3 1 5) (4 0 4)
               (5 2 11) (6 4 5) (7 5 13) (8 4 10)))
  (let ((source (number-sequence 0 (1- (car row)))) (calls 0))
    (setcdr (last source) source)
    (qa (condition-case e (plist-put source 'missing 42)
          (circular-list (car (cadr e)))) (cadr row))
    (qa (condition-case e
            (plist-put source 'missing 42 (lambda (&rest args) (setq calls (1+ calls)) nil))
          (circular-list (car (cadr e)))) (cadr row))
    (qa calls (nth 2 row))
    (qa (car source) 0)
    (qa (eq (plist-put source 0 'updated) source) t)
    (qa (plist-get source 0) (if (= (car row) 1) nil 'updated))))
(dolist (source '(atom (a) (a . tail)))
  (let ((calls 0))
    (qa-err-data (plist-put source 'a 42 (lambda (&rest args) (setq calls (1+ calls)) t))
                 'wrong-type-argument (list 'plistp source))
    (qa calls 0)
    (qa-err-data (plist-put source 'a 42 5) 'wrong-type-argument (list 'plistp source))))
(qa (plist-put nil 'a 42 5) '(a 42))
(qa-err (plist-put '(a 1) 'a 42 5) 'invalid-function)
;; A matching predicate may replace the value cell; only its new cell is written.
(let* ((old-values (list 'old)) (source (cons 'a old-values)))
  (qa (eq (plist-put source 'a 'result
                    (lambda (&rest args) (setcdr source (list 'replacement)) t)) source) t)
  (qa source '(a result))
  (qa old-values '(old)))
;; Replacement of the rest of the plist must affect subsequent lookup/update.
(let ((source (list 'a 'old)) (calls 0))
  (qa (eq (plist-put source 'target 'result
                    (lambda (key prop)
                      (setq calls (1+ calls))
                      (when (= calls 1) (setcdr source (list 'new 'target 'initial)))
                      (eq key prop))) source) t)
  (qa source '(a new target result))
  (qa calls 2))
(let ((source (list 'a 'old)) (calls 0))
  (qa (eq (plist-put source 'target 'result
                    (lambda (key prop)
                      (setq calls (1+ calls))
                      (when (= calls 1) (setcdr source (list 'replacement)))
                      nil)) source) t)
  (qa source '(a replacement target result))
  (qa calls 1))
;; Updating or extending a finite plist past the old 500000-pair cap works.
(let* ((source (make-list 1000000 nil)) (end (last source)) (tail (list 'target 'initial)))
  (setcdr end tail)
  (unwind-protect
      (progn
        (qa (eq (plist-put source 'target 'updated) source) t)
        (qa (cadr tail) 'updated)
        (qa (eq (plist-put source 'new 'value) source) t)
        (qa (cddr tail) '(new value))
        (qa (length source) 1000004))
    (setcdr end source)))
(garbage-collect)
(princ (qa--done))
