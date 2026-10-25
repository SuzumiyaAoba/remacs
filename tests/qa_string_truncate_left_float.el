;;; -*- lexical-binding: nil; -*-
;; A float LENGTH is accepted on string-truncate-left's early return (#397).
(qa (string-truncate-left "abc" 3.0) "abc")
(princ (qa--done))
