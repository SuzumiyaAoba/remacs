;;; -*- lexical-binding: nil; -*-
;; expand-file-name dispatches NAME handlers before local normalization (#449).
(qa (let ((file-name-handler-alist
           (cons (cons "\\`magic:"
                       (lambda (operation _name &optional _directory)
                         (when (eq operation 'expand-file-name)
                           "handler-result")))
                 file-name-handler-alist)))
      (expand-file-name "magic:dir/file" "/tmp"))
    "handler-result")
(princ (qa--done))
