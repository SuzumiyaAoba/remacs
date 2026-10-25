;;; -*- lexical-binding: nil; -*-
;; plist-put validates complete pairs before calling its predicate (#510).
(qa (let ((calls 0) (plist (list 'a)))
      (list (condition-case err
                (plist-put plist 'a 42
                           (lambda (&rest _) (setq calls (1+ calls)) t))
              (error (car err)))
            calls))
    '(wrong-type-argument 0))
(qa-err-data (plist-put '(a) 'a 42 5) 'wrong-type-argument
             '(plistp (a)))
(qa (let ((plist (list 'a 'old)) (calls 0))
      (plist-put plist 'a 'new
                 (lambda (key prop) (setq calls (1+ calls)) (eq key prop)))
      (list plist calls))
    '((a new) 1))
(princ (qa--done))
