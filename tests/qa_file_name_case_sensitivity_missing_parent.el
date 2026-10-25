;;; -*- lexical-binding: nil; -*-
;; file-name-case-insensitive-p walks up to an existing ancestor (#463).
(let* ((base temporary-file-directory)
       (missing (make-temp-name (expand-file-name "remacs-missing-" base)))
       (path (concat missing "/child/file")))
  (qa (file-name-case-insensitive-p path)
      (file-name-case-insensitive-p base)))
(princ (qa--done))
