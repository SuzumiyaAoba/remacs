;;; -*- lexical-binding: nil; -*-
;; assoc-default has no fixed predicate-call limit (#527).
(qa (let ((source (list (cons 'key 'result))) (calls 0))
      (setcdr source source)
      (list (assoc-default 'absent source
                           (lambda (&rest _)
                             (setq calls (1+ calls))
                             (> calls 500000)))
            calls))
    '(result 500001))
(qa (let ((source (make-list 500001 (cons 'key 'value))) (calls 0))
      (list (assoc-default 'absent source
                           (lambda (&rest _) (setq calls (1+ calls)) nil))
            calls))
    '(nil 500001))
(princ (qa--done))
