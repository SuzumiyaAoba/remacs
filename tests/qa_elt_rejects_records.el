;;; -*- lexical-binding: nil; -*-
;; elt rejects ordinary records before validating the index (#482).
(let ((record (record 'tag 1)))
  (qa (condition-case err (elt record 0)
        (error (list (car err) (cadr err) (recordp (caddr err)))))
      '(wrong-type-argument sequencep t))
  (qa (condition-case err (elt record 'invalid-index)
        (error (list (car err) (cadr err) (recordp (caddr err)))))
      '(wrong-type-argument sequencep t)))
(princ (qa--done))
