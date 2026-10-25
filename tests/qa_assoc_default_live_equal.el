;;; -*- lexical-binding: nil; -*-
;; assoc-default TEST=nil calls the live `equal' function cell (#526).
(qa (let ((original (symbol-function 'equal)))
      (unwind-protect
          (progn
            (fset 'equal (lambda (&rest _) t))
            (assoc-default 'x (list (cons 'unrelated 'value)))
            )
        (fset 'equal original)))
    'value)
(qa (assoc-default '(a) (list (cons (list 'a) 'value))) 'value)
(princ (qa--done))
