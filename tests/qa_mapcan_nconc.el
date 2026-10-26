;;; -*- lexical-binding: nil; -*-
;; mapcan uses Fnconc after collecting every callback result (#536).
(let ((cycle (list 'a)) (calls nil))
  (setcdr cycle cycle)
  (qa (condition-case e
          (mapcan (lambda (value) (push value calls) value)
                  (list cycle nil))
        (error (list (car e) (eq (cadr e) cycle))))
      '(circular-list t))
  (qa (length calls) 2)
  (qa (eq (cadr calls) cycle) t)
  ;; The final result is never traversed, even when circular.
  (qa (eq (mapcan #'identity (list nil cycle)) cycle) t)
  (let* ((head (list 'head)) (result (mapcan #'identity (list head cycle))))
    (qa (eq result head) t)
    (qa (eq (cdr head) cycle) t)))
;; On intermediate invalid objects, nconc connects before checking type.
(dolist (bad '(invalid 42 [a]))
  (let ((head (list 'head)) (calls nil))
    (qa (condition-case e
            (mapcan (lambda (value) (push value calls) value)
                    (list head bad '(final)))
          (error (list (car e) (cadr e) (eq (caddr e) bad))))
        '(wrong-type-argument consp t))
    (qa (eq (cdr head) bad) t)
    (qa (length calls) 3)))
(let* ((head (cons 'head 'discarded))
       (result (mapcan #'identity (list head nil 'tail))))
  (qa (eq result head) t)
  (qa result '(head . tail)))
(qa (mapcan #'identity '((a . discarded) (b . discarded) nil)) '(a b))
(qa (mapcan #'identity '(nil nil final)) 'final)
(qa (mapcan (lambda (_) (error "unused")) nil) nil)
;; The C primitive call does not resolve a redefined nconc function cell.
(let ((saved (symbol-function 'nconc)))
  (unwind-protect
      (progn
        (fset 'nconc (lambda (&rest _) (error "replacement nconc")))
        (qa (mapcan #'identity (list (list 'a) (list 'b))) '(a b)))
    (fset 'nconc saved)))
(princ (qa--done))
