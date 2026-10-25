;;; -*- lexical-binding: nil; -*-
;; concat rejects bool-vectors as non-character sequences (#435).
(dolist (vector (list (make-bool-vector 0 nil) (make-bool-vector 1 t)))
  (qa (condition-case err (concat vector)
        (error (list (car err) (cadr err) (bool-vector-p (caddr err)))))
      '(wrong-type-argument sequencep t)))
(princ (qa--done))
