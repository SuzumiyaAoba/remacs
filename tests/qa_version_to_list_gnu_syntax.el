;;; -*- lexical-binding: nil; -*-
;; version-to-list honors GNU prerelease syntax and configuration (#581).
(qa
 (mapcar (lambda (version)
           (list (version-to-list version) (version< version "1.0")))
         '("1.0pre" "1.0beta" "1.0-git" "1.0rc1" "1.0pre1"))
 '(((1 0 -1) t) ((1 0 -2) t) ((1 0 -4) t)
   ((1 0 -1 1) t) ((1 0 -1 1) t)))
(qa (version-to-list ".5") '(0 5))
(qa
 (let ((version-regexp-alist
        (cons (cons "^dev$" -5) version-regexp-alist)))
   (version-to-list "1.0dev"))
 '(1 0 -5))
(qa
 (let ((version-separator "_"))
   (version-to-list "1_2"))
 '(1 2))
(qa-err (version-to-list "1.0xyz") 'error)
(princ (qa--done))
