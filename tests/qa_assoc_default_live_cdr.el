;;; -*- lexical-binding: nil; -*-
;; assoc-default follows the current alist cdr after TEST (#525).
(qa (let ((alist (list (cons 'first 'old))) (calls 0))
      (list (assoc-default 'target alist
                           (lambda (key wanted)
                             (setq calls (1+ calls))
                             (when (= calls 1)
                               (setcdr alist (list (cons 'target 'result))))
                             (eq key wanted)))
            calls))
    '(result 2))
(qa (let ((alist (list (cons 'key 'old))))
      (assoc-default 'key alist
                     (lambda (key wanted)
                       (setcdr (car alist) 'updated)
                       (eq key wanted))))
    'updated)
(princ (qa--done))
