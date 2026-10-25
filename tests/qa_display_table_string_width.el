;;; -*- lexical-binding: nil; -*-
;; char-width and string-width account for display-table replacements (#440).
(let ((old standard-display-table)
      (table (make-display-table)))
  (unwind-protect
      (progn
        (aset table ?a [120 120 120])
        (setq standard-display-table table)
        (with-temp-buffer
          (qa (char-width ?a) 3)
          (qa (string-width "a") 3)))
    (setq standard-display-table old)))
(princ (qa--done))
