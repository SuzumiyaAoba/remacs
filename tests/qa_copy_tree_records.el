;;; -*- lexical-binding: nil; -*-
;; copy-tree recursively copies ordinary records when VECTORS-AND-RECORDS is non-nil (#468).
(let* ((child (list 'a))
       (record (record 'tag child))
       (copied (copy-tree record t)))
  (qa (list (eq record copied)
            (eq child (aref copied 1))
            (eq child (aref record 1))
            (equal (aref copied 1) '(a)))
      '(nil nil t t))
  (qa (eq record (copy-tree record)) t))
(princ (qa--done))
