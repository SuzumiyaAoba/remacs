;;; -*- lexical-binding: nil; -*-
;; A circular fontset alias alist must not trap new-fontset in its scan.
(when qa--remacs
  (let ((fontset-alias-alist nil)
        (aliases (list (cons "not-an-alias" "not-an-alias"))))
    (setcdr aliases aliases)
    (setq fontset-alias-alist aliases)
    (qa-err (new-fontset "fontset-unregistered" nil) 'circular-list)))

(princ (qa--done))
