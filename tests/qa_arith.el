;; Arithmetic & number predicates — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - (/ 1.0 0) => 1.0e+INF; (/ 1.0 0.0) => 1.0e+INF;
;;     (/ 1 0) => arith-error; (/ 1 0.0) => 1.0e+INF.
;;   - (sqrt -1) => 0.0e+NaN, (log 0) => -1.0e+INF, (log -1) => 0.0e+NaN.
;;   - (% 5.0 2) => wrong-type-argument (integers only).
;;   - round is banker's rounding: (round 2.5)=>2, (round 3.5)=>4.
;;   - (expt 0 0) => 1; (expt 2 -2) => 0.25.
;;   - (number-to-string N BASE) has no BASE arg in 31.1.
;;   - (eql 1 1.0) => nil but (= 1 1.0) => t.
;;   - (lsh 4 -1) => 2 (negative count shifts right, logical).
;;   - (logcount -2) => 1 (count of 0-bits for negative inputs).

;; == predicates ==
(qa (numberp 5) t)
(qa (numberp 1.5) t)
(qa (numberp "5") nil)
(qa (number-or-marker-p 3) t)
(qa (integerp 5) t)
(qa (integerp 1.0) nil)
(qa (floatp 1.5) t)
(qa (floatp 5) nil)
(qa (natnump 0) t)
(qa (natnump -1) nil)
(qa (wholenump 3) t)
(qa (zerop 0) t)
(qa (zerop 0.0) t)
(qa (oddp 3) t)
(qa (evenp 4) t)
(qa (plusp 0) nil)
(qa (plusp 1) t)
(qa (minusp -1) t)
(qa (fixnump 5) t)
(qa (bignump 5) nil)
(qa (characterp ?a) t)
(qa (characterp 97) t)
(qa (eq ?a 97) t)

;; == comparison ==
(qa (= 1 1.0) t)
(qa (eql 1 1.0) nil)
(qa (eql 1.0 1.0) t)
(qa (= 2 2 2) t)
(qa (/= 1 2) t)
(qa (< 1 2 3) t)
(qa (< 1 3 2) nil)
(qa (<= 1 1 2) t)
(qa (> 3 2 1) t)
(qa (>= 2 2 1) t)
(qa-err (= 1 "1") 'wrong-type-argument)

;; == + - * / ==
(qa (+) 0)
(qa (*) 1)
(qa (+ 1 2 3) 6)
(qa (+ 1 2.5) 3.5)
(qa (- 10 3 2) 5)
(qa (- 5) -5)
(qa (* 2 3 4) 24)
(qa (/ 6 3) 2)
(qa (/ 7 2) 3)
(qa (/ 7.0 2) 3.5)
(qa (/ 1 2) 0)
(qa (/ 1.0 0) 1.0e+INF)
(qa (/ -1.0 0) -1.0e+INF)
(qa (/ 1 0.0) 1.0e+INF)
(qa-err (/ 1 0) 'arith-error)
(qa (1+ 5) 6)
(qa (1+ 2.5) 3.5)
(qa (1- 5) 4)

;; == % / mod ==
(qa (% 7 3) 1)
(qa (% -7 3) -1)
(qa (% 7 -3) 1)
(qa (mod 7 3) 1)
(qa (mod -7 3) 2)
(qa (mod 7 -3) -2)
(qa (mod 7.5 2) 1.5)
(qa-err (% 5.0 2) 'wrong-type-argument)
(qa-err (mod 5 0) 'arith-error)

;; == floor / ceiling / round / truncate ==
(qa (floor 7.9) 7)
(qa (floor -7.9) -8)
(qa (floor 7) 7)
(qa (floor 7.5 2) 3)
(qa (ceiling 7.1) 8)
(qa (ceiling -7.9) -7)
(qa (round 2.5) 2)
(qa (round 3.5) 4)
(qa (round -2.5) -2)
(qa (truncate 7.9) 7)
(qa (truncate -7.9) -7)
(qa (ffloor 7.2) 7.0)
(qa (fceiling 7.2) 8.0)
(qa (fround 2.5) 2.0)
(qa (ftruncate 7.9) 7.0)

;; == min / max / abs ==
(qa (min 3 1 2) 1)
(qa (max 3 1 2) 3)
(qa (min 1 1.5 0) 0)
(qa (max 1.5 2) 2)
(qa (abs -5) 5)
(qa (abs -5.5) 5.5)
(qa (abs 0) 0)

;; == expt / sqrt / exp / log ==
(qa (expt 2 10) 1024)
(qa (expt 0 0) 1)
(qa (expt 2 -2) 0.25)
(qa (expt 2.0 3) 8.0)
(qa (expt 4 0.5) 2.0)
(qa (sqrt 16) 4.0)
(qa (sqrt 2) 1.4142135623730951)
(qa (sqrt -1) 0.0e+NaN)
(qa (exp 0) 1.0)
(qa (exp 1) 2.718281828459045)
(qa (log 1) 0.0)
(qa (log 0) -1.0e+INF)
(qa (log -1) 0.0e+NaN)
(qa (log 100 10) 2.0)
(qa (expt 2 62) 4611686018427387904)

;; == trig ==
(qa (sin 0) 0.0)
(qa (cos 0) 1.0)
(qa (atan 0) 0.0)
(qa (= pi 3.141592653589793) t)
(qa (asin 1) 1.5707963267948966)
(qa (acos 1) 0.0)
(qa (isnan (/ 0.0 0.0)) t)
(qa (isnan 1.0) nil)

;; == bitwise ==
(qa (logand 6 3) 2)
(qa (logior 6 3) 7)
(qa (logxor 6 3) 5)
(qa (lognot 0) -1)
(qa (ash 1 4) 16)
(qa (ash 1 -1) 0)
(qa (ash -8 -2) -2)
(qa (lsh 4 -1) 2)
(qa (lsh -1 1) -2)
(qa (ash 0 1000000000) 0)
(qa (lsh 0 1000000000) 0)
(qa-err (ash 0 most-positive-fixnum) 'overflow-error)
(qa-err (lsh 0 most-positive-fixnum) 'overflow-error)
(qa (lsh 0 (- most-positive-fixnum)) 0)
(qa (logcount 13) 3)
(qa (logcount -1) 0)
(qa (logcount -2) 1)

;; == misc ==
(qa (float 3) 3.0)
(qa (frexp 8.0) '(0.5 . 4))
(qa (copysign 2.0 -1.0) -2.0)
(qa (number-to-string 42) "42")
(qa (number-to-string 2.5) "2.5")
(qa-err (number-to-string 255 16) 'wrong-number-of-arguments)
(qa (string-to-number "42") 42)
(qa (string-to-number "2.5") 2.5)
(qa (string-to-number "abc") 0)
(qa (string-to-number "1e3") 1000.0)
(qa (string-to-number "ff" 16) 255)
(qa-err (+ 1 "x") 'wrong-type-argument)
(qa-err (* 'a 2) 'wrong-type-argument)
(qa-err (ash 1 'x) 'wrong-type-argument)

(princ (qa--done))
