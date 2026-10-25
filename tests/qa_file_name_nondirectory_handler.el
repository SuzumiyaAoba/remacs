;;; -*- lexical-binding: nil; -*-
;; file-name-nondirectory dispatches its operation-specific handler (#445).
(qa (let ((file-name-handler-alist
           (cons (cons "\\`magic:"
                       (lambda (operation _filename)
                         (when (eq operation 'file-name-nondirectory)
                           "handler-result")))
                 file-name-handler-alist)))
      (file-name-nondirectory "magic:dir/file"))
    "handler-result")
(princ (qa--done))
