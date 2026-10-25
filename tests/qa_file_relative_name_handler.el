;;; -*- lexical-binding: nil; -*-
;; file-relative-name uses handler-aware expand-file-name (#452).
(qa (let ((default-directory "/base/")
       (file-name-handler-alist
        (cons (cons "\\`magic:"
                    (lambda (operation _name &optional _directory)
                      (when (eq operation 'expand-file-name)
                        "/base/target")))
              file-name-handler-alist)))
      (file-relative-name "magic:file" "/base/"))
    "target")
(princ (qa--done))
