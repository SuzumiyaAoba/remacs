;;; -*- lexical-binding: nil; -*-
;; Range endpoints are validated as characters before conversion (#498).
(qa-err-data (modify-syntax-entry (cons 4294967361 4294967361) "w")
             'wrong-type-argument '(characterp 4294967361))
(qa-err-data (modify-syntax-entry (cons -1 5) "w")
             'wrong-type-argument '(characterp -1))
(qa-err-data (modify-syntax-entry (cons 65 4294967361) "w")
             'wrong-type-argument '(characterp 4294967361))
(qa (let ((marker (copy-marker 1)))
      (unwind-protect
          (condition-case err
              (modify-syntax-entry (cons marker marker) "w")
            (wrong-type-argument
             (list (car err) (cadr err) (eq (caddr err) marker))))
        (set-marker marker nil)))
    '(wrong-type-argument characterp t))
(qa (let ((table (make-syntax-table)))
      (modify-syntax-entry (cons 65 66) "w" table)
      (with-syntax-table table (list (char-syntax 65) (char-syntax 66))))
    '(?w ?w))
(princ (qa--done))
