;;; -*- lexical-binding: nil; -*-
;; Text-property scan functions reject detached markers (#480).
(dolist (form (list (lambda () (text-property-any (make-marker) 1 'x nil))
                    (lambda () (next-single-property-change (make-marker) 'x))
                    (lambda () (previous-single-property-change (make-marker) 'x))))
  (qa (condition-case err (funcall form) (error (car err))) 'error))
(princ (qa--done))
