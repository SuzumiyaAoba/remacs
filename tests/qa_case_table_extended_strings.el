;;; -*- lexical-binding: nil; -*-
;; String case conversion consults case tables using Emacs character codes (#441).
(qa (with-temp-buffer
      (let ((table (make-char-table 'case-table nil)))
        (aset table #x110001 #x110002)
        (set-case-table table)
        (list (downcase #x110001)
              (string-to-list (downcase (string #x110001)))
              (string-to-list (upcase (string #x110002)))))
    '(1114114 (1114114) (1114113)))
(princ (qa--done))
