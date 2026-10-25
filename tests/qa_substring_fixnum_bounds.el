;;; -*- lexical-binding: nil; -*-
;; Non-nil substring bounds must be fixnums (#546).
(qa-err-data (substring "abc" 0 (expt 2 64)) 'wrong-type-argument
             '(integerp 18446744073709551616))
(qa-err-data (substring-no-properties "abc" (expt 2 64))
             'wrong-type-argument '(integerp 18446744073709551616))
(qa-err-data (substring "abc" (- (expt 2 64))) 'wrong-type-argument
             '(integerp -18446744073709551616))
(qa-err-data (substring [a b c] 0 (expt 2 64)) 'wrong-type-argument
             '(integerp 18446744073709551616))

;; Check both bound types before reporting an otherwise invalid range.
(qa-err-data (substring "abc" 100 (expt 2 64)) 'wrong-type-argument
             '(integerp 18446744073709551616))
(qa-err-data (substring "abc" (expt 2 64) (expt 2 65))
             'wrong-type-argument '(integerp 18446744073709551616))
(qa (substring "abc" nil -1) "ab")
(princ (qa--done))
