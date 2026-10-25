;;; -*- lexical-binding: nil; -*-
;; Filename extension helpers compose handler-aware operations (#446).
(let ((file-name-handler-alist
       (cons (cons "\\`magic:"
                   (lambda (operation filename &rest _)
                     (pcase operation
                       ('file-name-sans-versions filename)
                       ('file-name-nondirectory "virtual.tar.gz")
                       ('file-name-directory "remote/"))))
             file-name-handler-alist)))
  (qa (list (file-name-extension "magic:dir/original.txt")
            (file-name-sans-extension "magic:dir/original.txt")
            (file-name-base "magic:dir/original.txt"))
      '("gz" "remote/virtual.tar" "virtual.tar")))
(princ (qa--done))
