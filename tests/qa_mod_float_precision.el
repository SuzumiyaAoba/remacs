;;; -*- lexical-binding: nil; -*-
;; Floating mod follows fmod plus one sign adjustment (#545).
(qa (mod -1 -1.0e+INF) -1.0)
(qa (mod -0.1 -1.0) -0.1)
(qa (mod -1.0e-300 -1.0) -1.0e-300)
(qa (mod 1.0 -2.0) -1.0)
(qa (mod -1.0 2.0) 1.0)
(qa (prin1-to-string (mod -0.0 1.0)) "-0.0")
(qa (prin1-to-string (mod -2.0 1.0)) "-0.0")
(princ (qa--done))
