;;; -*- lexical-binding: nil; -*-
;; member-if follows the current cons cdr after each predicate call (#514).
(qa (let ((list (list 'a 'old)) (calls 0))
      (list (car-safe
             (member-if (lambda (item)
                          (setq calls (1+ calls))
                          (when (= calls 1) (setcdr list (list 'target)))
                          (eq item 'target))
                        list))
            calls))
    '(target 2))
(princ (qa--done))
