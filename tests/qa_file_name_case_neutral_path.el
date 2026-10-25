;;; -*- lexical-binding: nil; -*-
;; A case-neutral filename cannot prove an unknown filesystem is insensitive (#467).
(let ((base temporary-file-directory))
  (qa (file-name-case-insensitive-p (expand-file-name "123" base))
      (file-name-case-insensitive-p base)))
(princ (qa--done))
