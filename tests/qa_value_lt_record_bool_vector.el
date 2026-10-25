;;; -*- lexical-binding: nil; -*-
;; value< orders records and bool-vectors lexicographically (#565).
(qa (value< (record 'tag 1) (record 'tag 2)) t)
(qa (value< (record 'tag 2) (record 'tag 1)) nil)
(qa (value< (bool-vector nil t) (bool-vector t nil)) t)
(qa (value< (bool-vector t nil) (bool-vector nil t)) nil)
(qa-err (value< (record 'tag) (bool-vector)) 'type-mismatch)
(qa (value< (list (record 'tag 1) 0)
            (list (record 'tag 2) 0))
    t)
(princ (qa--done))
