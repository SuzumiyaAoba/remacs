;;; -*- lexical-binding: nil; -*-
;; file-accessible-directory-p passes its own operation to handlers (#517).
(qa (let* ((handler
            (lambda (operation &rest args)
              (cond ((eq operation 'file-accessible-directory-p) t)
                    ((eq operation 'file-directory-p) nil)
                    (t (car args)))))
           (file-name-handler-alist
            (cons (cons "^/qa-vfs/" handler) file-name-handler-alist)))
      (list (file-accessible-directory-p "/qa-vfs/x")
            (file-directory-p "/qa-vfs/x")))
    '(t nil))
(princ (qa--done))
