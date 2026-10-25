;;; -*- lexical-binding: nil; -*-
;; concat rejects bool-vector constructor results, including empty ones (#549).
(dolist (value (list (make-bool-vector 0 nil) (bool-vector t nil)))
  (qa (condition-case err (concat value)
        (error (list (car err) (cadr err) (bool-vector-p (caddr err)))))
    '(wrong-type-argument sequencep t)))
(princ (qa--done))
