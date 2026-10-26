;;; -*- lexical-binding: nil; -*-
;; #678: GNU's zero-length non-string input leaves its result list empty.
(dolist (omit '(nil t))
  (dolist (keep '(nil t))
    (qa (string-lines nil omit keep) nil)))
(princ (qa--done))
