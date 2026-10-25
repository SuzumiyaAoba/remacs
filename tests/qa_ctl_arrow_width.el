;;; -*- lexical-binding: nil; -*-
;; C0 display width follows ctl-arrow for character, string, and buffer APIs (#501).
(qa (with-temp-buffer
      (insert (string 1))
      (let ((ctl-arrow nil))
        (list (char-width 1)
              (string-width (string 1))
              (current-column)
              (move-to-column 4)
              (point)
              (current-column))))
    '(4 4 4 4 2 4))
(qa (with-temp-buffer
      (insert (string 1))
      (let ((ctl-arrow t))
        (list (char-width 1) (string-width (string 1)) (current-column))))
    '(2 2 2))
(princ (qa--done))
