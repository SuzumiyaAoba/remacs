;;; -*- lexical-binding: nil; -*-
;; elt rejects ordinary records before validating the index (#482).
(let ((record (record 'tag 1)))
  (dolist (index '(0 invalid-index))
    (qa (condition-case err (elt record index)
          (error (list (car err) (cadr err) (recordp (caddr err)))))
        '(wrong-type-argument sequencep t)))
(princ (qa--done))
