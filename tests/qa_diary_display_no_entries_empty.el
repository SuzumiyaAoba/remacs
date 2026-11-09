;;; -*- lexical-binding: nil; -*-
;; Multiple diary entries with empty text still mean there are no entries.
(require 'diary-lib)
(qa
 (let ((diary-show-holidays-flag nil)
       (diary-entries-list '(((1 1 2026) "") ((1 2 2026) "")))
       (diary--date-string "today"))
   (diary-display-no-entries))
 '(t . "today"))
(princ (qa--done))
