;;; -*- lexical-binding: nil; -*-
;; Ignored missing specifiers remain a single element when SPLIT is non-nil.
(require 'format-spec)
(qa (format-spec "%x" nil 'ignore t) '("%x"))
(qa (format-spec "before %x after" nil 'ignore t)
    '("before " "%x" " after"))
(qa (format-spec "before %x after" nil t t)
    '("before " "%x" " after"))
(princ (qa--done))
