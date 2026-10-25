;;; -*- lexical-binding: nil; -*-
;; file-system-info dispatches through filename handlers (#512).
(qa (let ((file-name-handler-alist
           (cons (cons "^/qa-vfs/"
                       (lambda (operation &rest args)
                         (if (eq operation 'file-system-info)
                             '(10 9 8)
                           (car args))))
                 file-name-handler-alist)))
      (file-system-info "/qa-vfs/file"))
    '(10 9 8))
(princ (qa--done))
