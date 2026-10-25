;;; -*- lexical-binding: nil; -*-
;; Column, indentation, and rectangle APIs use GNU's sane tab width (#502).
(qa (with-temp-buffer
      (insert "\t")
      (let ((tab-width 1001))
        (list (current-column) (move-to-column 1001) (point) (current-column))))
    '(8 8 2 8))
(qa (with-temp-buffer
      (insert "\t")
      (let ((tab-width -1))
        (list (current-column) (move-to-column 8) (current-column))))
    '(8 8 8))
(qa (with-temp-buffer
      (let ((tab-width 0) (indent-tabs-mode t))
        (let ((result (indent-to 8)))
          (list result (buffer-string) (point)))))
    '(8 "\t" 2))
(qa (with-temp-buffer
      (insert "\tX\n123456789Y\n")
      (let ((tab-width 0))
        (set-mark 5)
        (goto-char 1)
        (delete-rectangle (region-beginning) (region-end))
        (buffer-string)))
    "       X\n23456789Y\n")
(princ (qa--done))
