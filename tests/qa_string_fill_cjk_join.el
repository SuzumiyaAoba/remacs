;;; -*- lexical-binding: nil; -*-
;; string-fill omits spaces between CJK words across newline (#438).
(qa (string-fill "日\n本" 80) "日本")
(princ (qa--done))
