;;; -*- lexical-binding: nil; -*-
;; assq-delete-all and rassq-delete-all have no finite-list guard (#528).
(qa (let ((source (make-list 500001 nil)))
      (list (eq (assq-delete-all 'missing source) source)
            (eq (rassq-delete-all 'missing source) source)))
    '(t t))
(princ (qa--done))
