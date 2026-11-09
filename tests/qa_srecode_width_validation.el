;;; -*- lexical-binding: nil; -*-
;; Width strings must be entirely numeric before padding or truncation.
(load-file (expand-file-name "../lisp/srecode-insert.el"
                             (file-name-directory load-file-name)))
(let ((inserter (make-instance 'srecode-template-inserter-width)))
  (cl-letf (((symbol-function 'srecode-insert-report-error)
             (lambda (_dictionary format &rest args)
               (apply #'error format args))))
    (qa-err
     (srecode-insert-variable-secondname-handler
      inserter nil "Hello" "bogus:right")
     'error)
    (qa-err
     (srecode-insert-variable-secondname-handler
      inserter nil "Hello" "4junk:right")
     'error)
    (qa (srecode-insert-variable-secondname-handler
         inserter nil "Hello" "5:right")
        "Hello")))
(princ (qa--done))
