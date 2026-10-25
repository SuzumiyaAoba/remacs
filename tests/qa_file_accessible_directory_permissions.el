;;; -*- lexical-binding: nil; -*-
;; file-accessible-directory-p checks directory read/search permissions (#519).
(let ((directory (make-temp-file "qa-accessible-directory-" t)))
  (unwind-protect
      (progn
        (set-file-modes directory 0)
        (qa (list (file-directory-p directory)
                  (file-accessible-directory-p directory))
            '(t nil)))
    (set-file-modes directory #o700)
    (delete-directory directory)))
(princ (qa--done))
