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
(princ (qa--done))
