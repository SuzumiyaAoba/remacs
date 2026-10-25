;;; -*- lexical-binding: nil; -*-
;; Directory listing and completion dispatch their matching handler ops (#513).
(qa (let ((file-name-handler-alist
           (cons
            (cons "^/qa-vfs/"
                  (lambda (operation &rest args)
                    (cond ((eq operation 'directory-files) '("virtual-entry"))
                          ((eq operation 'directory-files-and-attributes)
                           '(("virtual-entry" . (attributes))))
                          ((eq operation 'file-name-completion) "virtual")
                          ((eq operation 'file-name-all-completions) '("all"))
                          (t (car args)))))
            file-name-handler-alist)))
      (list (directory-files "/qa-vfs/")
            (directory-files-and-attributes "/qa-vfs/")
            (file-name-completion "x" "/qa-vfs/")
            (file-name-all-completions "x" "/qa-vfs/")))
    '(("virtual-entry")
      (("virtual-entry" . (attributes)))
      "virtual"
      ("all")))
(princ (qa--done))
