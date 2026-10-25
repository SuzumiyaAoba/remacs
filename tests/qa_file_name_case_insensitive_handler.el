;;; -*- lexical-binding: nil; -*-
;; file-name-case-insensitive-p dispatches its operation handler (#461).
(qa (let ((file-name-handler-alist
           (cons (cons "\\`magic:"
                       (lambda (operation _filename)
                         (when (eq operation 'file-name-case-insensitive-p)
                           'from-handler)))
                 file-name-handler-alist)))
      (file-name-case-insensitive-p "magic:remote-name"))
    'from-handler)
(princ (qa--done))
