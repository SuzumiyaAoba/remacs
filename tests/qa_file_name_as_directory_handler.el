;;; -*- lexical-binding: nil; -*-
;; file-name-as-directory calls its operation-specific handler (#447).
(qa (let ((file-name-handler-alist
           (cons (cons "\\`magic:"
                       (lambda (operation _filename)
                         (when (eq operation 'file-name-as-directory)
                           "handler-directory/")))
                 file-name-handler-alist)))
      (file-name-as-directory "magic:remote/path"))
    "handler-directory/")
(princ (qa--done))
