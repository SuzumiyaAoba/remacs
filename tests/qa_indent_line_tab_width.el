;;; -*- lexical-binding: nil; -*-
;; indent-line-to keeps GNU behavior for invalid tab-width values (#507).
(qa (with-temp-buffer
      (insert "X")
      (let ((tab-width 0) (indent-tabs-mode t))
        (condition-case err (indent-line-to 8)
          (arith-error (car err)))))
    'arith-error)
(qa (with-temp-buffer
      (insert "X")
      (let ((tab-width -1) (indent-tabs-mode t))
        (list (indent-line-to 8) (buffer-string) (current-column))))
    '(8 "\tX" 8))
(qa (with-temp-buffer
      (insert "X")
      (let ((tab-width 1001) (indent-tabs-mode t))
        (list (indent-line-to 8) (buffer-string) (current-column))))
    '(8 "\tX" 8))
(princ (qa--done))
