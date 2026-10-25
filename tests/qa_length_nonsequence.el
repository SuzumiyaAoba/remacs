;;; -*- lexical-binding: nil; -*-
;; Native window snapshots are not record sequences (#358).
(let ((snapshot (current-window-configuration)))
  (dolist (op '(length= length< length>))
    (dolist (limit '(0 100))
      (qa (condition-case e (funcall op snapshot limit)
            (error (list (car e) (cadr e) (eq (caddr e) snapshot))))
          '(wrong-type-argument sequencep t)))
    ;; The length argument is checked before the sequence argument.
    (qa-err-data (funcall op snapshot 'bad)
                 'wrong-type-argument '(fixnump bad))
    ;; Ordinary records still have sequence length.
    (qa (funcall op (record 'test 1 2) 3)
        (eq op 'length=))))
;; A negative length= limit returns nil before checking the sequence (#359).
(dolist (object (list 42 'not-a-sequence (make-hash-table)))
  (qa (length= object -1) nil)
  (qa (length= object most-negative-fixnum) nil)
  (qa-err-data (length= object 0) 'wrong-type-argument
               (list 'sequencep object))
  (qa-err-data (length< object -1) 'wrong-type-argument
               (list 'sequencep object))
  (qa-err-data (length> object -1) 'wrong-type-argument
               (list 'sequencep object)))
(princ (qa--done))
