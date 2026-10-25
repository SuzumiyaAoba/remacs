;;; -*- lexical-binding: nil; -*-
;; value< and default sort use exact integer/float numeric ordering (#558).
(qa (value< (expt 2 65) (expt 2 70)) t)
(qa (value< 9007199254740992.0 9007199254740993) t)
(qa (sort (list (expt 2 70) 1 (expt 2 65)) nil)
    '(1 36893488147419103232 1180591620717411303424))
(qa (value< 0.0e+NaN 1) nil)
(qa-err (value< (expt 2 70) 'symbol) 'type-mismatch)
(princ (qa--done))
