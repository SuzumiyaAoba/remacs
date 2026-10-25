;;; -*- lexical-binding: nil; -*-
;; string-lines computes length before signaling stringp on bool-vectors (#411).
(qa (condition-case err (string-lines (make-bool-vector 2 nil))
      (error (list (car err) (cadr err) (bool-vector-p (caddr err)))))
    '(wrong-type-argument stringp t))
(princ (qa--done))
