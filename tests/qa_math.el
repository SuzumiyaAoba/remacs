;; Numeric semantics — GNU 31.1 pins:
;;   - `mod' returns a result with the DIVISOR's sign: (mod -7 3) → 2,
;;     (mod 7 -3) → -2.  `%' (rem) has the DIVIDEND's sign: (% -7 3) → -1.
;;   - `frexp' returns (FRACTION . EXPONENT) with fraction in [0.5, 1);
;;     frexp of 0.0 is (0.0 . 0); negatives keep the sign in the fraction.
;;   - `logb' is 1-arg in 31.1 (exponent of float): logb 8.0 → 3, logb 0.5
;;     → -1, logb 0.0 → -INF (no error).  logb 1e300 → 996.
;;   - `min'/`max' return the FIRST argument on ties, preserving its type:
;;     (min 1 1.0) → int 1; (min 1.0 1) → float 1.0.
;;   - `abs -0.0' → 0.0 (positive zero), not -0.0.
;;   - `float' on a bignum yields a float (may lose precision):
;;     (float (1+ most-positive-fixnum)) → 2.305843009213694e+18.
;;   - `string-to-number' parses a number PREFIX: "42junk" → 42, "5x" → 5,
;;     "abc" → 0.  A base argument reads in that radix.
;;   - Reader radix forms: #x1f, #o17, #b101, #16rff, 1e2, 1.5e-2.
;;     "-#x10" reads as the symbol `-' (radix not accepted after minus).
;;   - #. read-eval signals `invalid-read-syntax' under -Q (read-eval nil).
;;   - `logcount' counts 1 bits (positive) or 0 bits (negative): -1 → 0
;;     on a 62-bit fixnum system, -255 → 7.
;;   - `lsh' is a LOGICAL shift: (lsh -1 -1) → most-positive-fixnum,
;;     (lsh -4 -2) → 1152921504606846975.
;;   - `expt' int results for int args (expt 2 -1 → 0.5 float though),
;;     expt 0 0 → 1; sqrt on a negative → NaN (not an error).
;;   - `isqrt' does NOT exist in 31.1; `cl-gcd'/`cl-lcm' do.
;;   - Seeded cl-random: state vector is
;;     (vector 'cl-random-state-tag -1 30 SEED); known-seed sequence pins
;;     determinism; cl-copy-seq of the state + rebind replays the run.
(require 'cl-lib)

;; == mod vs % ==
(qa (mod 7 3) 1)
(qa (mod -7 3) 2)
(qa (mod 7 -3) -2)
(qa (mod -7 -3) -1)
(qa (% 7 3) 1)
(qa (% -7 3) -1)
(qa (% 7 -3) 1)
(qa (% -7 -3) -1)
(qa-err (mod 7 0) 'arith-error)
(qa-err (% 7 0) 'arith-error)
;; float mod allowed
(qa (mod 7.5 2) 1.5)
(qa (mod 7 2.5) 2.0)

;; == frexp / ldexp ==
(qa (frexp 8.0) '(0.5 . 4))
(qa (frexp 0.5) '(0.5 . 0))
(qa (frexp 0.0) '(0.0 . 0))
(qa (frexp -4.0) '(-0.5 . 3))
(qa (frexp 1.0) '(0.5 . 1))
(qa (ldexp 0.5 4) 8.0)
(qa (ldexp 1.0 0) 1.0)
(qa (ldexp -0.25 3) -2.0)
;; roundtrip: x = frexp-frac * 2^frexp-exp
(qa (let ((f (frexp 100.0)))
      (ldexp (car f) (cdr f)))
    100.0)

;; == logb (float exponent) ==
(qa (logb 8.0) 3)
(qa (logb 1.0) 0)
(qa (logb 0.5) -1)
(qa (logb 1e300) 996)
(qa (logb 0.0) -1.0e+INF)
(qa (logb -8.0) 3) ;; sign ignored

;; == min/max tie-return preserves FIRST arg's type ==
(qa (min 1 1.0) 1)
(qa (max 1 1.0) 1)
(qa (min 1.0 1) 1.0)
(qa (max 1.0 1) 1.0)
(qa (min 3 1 2) 1)
(qa (max 3 1 2) 3)
(qa (min 5) 5)
(qa (max -1 -2 -3) -1)
;; on floats: float result even when an int is smaller
(qa (min 1 0.5) 0.5)
(qa (floatp (min 1 0.5)) t)

;; == abs ==
(qa (abs -3) 3)
(qa (abs 3) 3)
(qa (abs -3.5) 3.5)
(qa (abs 0.0) 0.0)
(qa (abs -0.0) 0.0) ;; positive zero — loses the sign
(qa (eql (abs -0.0) -0.0) nil)
(qa (abs most-negative-fixnum) 2305843009213693952)

;; == float conversions ==
(qa (float 8) 8.0)
(qa (float -3) -3.0)
(qa (float 0) 0.0)
(qa (float (1+ most-positive-fixnum)) 2.305843009213694e+18)
(qa (floatp (float most-positive-fixnum)) t)

;; == number-to-string / string-to-number ==
(qa (number-to-string 42) "42")
(qa (number-to-string -1) "-1")
(qa (number-to-string 3.5) "3.5")
(qa (number-to-string 0) "0")
(qa (string-to-number "42") 42)
(qa (string-to-number "-7") -7)
(qa (string-to-number "3.5") 3.5)
(qa (string-to-number "  5") 5)
(qa (string-to-number "5x") 5)    ;; trailing junk ignored
(qa (string-to-number "42junk") 42)
(qa (string-to-number "abc") 0)
(qa (string-to-number "") 0)
(qa (string-to-number "  -12.5xy") -12.5)
(qa (string-to-number "1e3") 1000.0)
;; BASE argument reads in that radix
(qa (string-to-number "10" 16) 16)
(qa (string-to-number "ff" 16) 255)
(qa (string-to-number "101" 2) 5)
(qa (string-to-number "17" 8) 15)
;; base 16 accepts uppercase too
(qa (string-to-number "FF" 16) 255)
;; float strings still parse in base 10 form
(qa (string-to-number "1.5" 10) 1.5)

;; == reader radix forms ==
(qa (car (read-from-string "#x1f")) 31)
(qa (car (read-from-string "#o17")) 15)
(qa (car (read-from-string "#b101")) 5)
(qa (car (read-from-string "#16rff")) 255)
(qa (car (read-from-string "#2r1010")) 10)
(qa (car (read-from-string "#8r377")) 255)
(qa (car (read-from-string "1e2")) 100.0)
(qa (car (read-from-string "1.5e-2")) 0.015)
;; "-#x10" reads as the SYMBOL - (no radix after a minus)
(qa (car (read-from-string "-#x10")) '-)
;; #. read-eval is disabled by default under -Q (read-eval nil)
(qa-err (car (read-from-string "#.(+ 1 2)")) 'invalid-read-syntax)

;; == expt / sqrt ==
(qa (expt 2 10) 1024)
(qa (expt 2 -1) 0.5)
(qa (expt 0 0) 1)
(qa (expt -2 3) -8)
(qa (expt -2 2) 4)
(qa (expt 2.0 10) 1024.0)
(qa (expt 1.5 2) 2.25)
(qa (expt 0 -1) 1.0e+INF) ;; 0^-1 → INF float, not arith-error
(qa (sqrt 4) 2.0)
(qa (sqrt 2.0) 1.4142135623730951)
(qa (sqrt 0) 0.0)
(qa (sqrt -1) 0.0e+NaN) ;; NaN, no error

;; == mixed comparisons ==
(qa (= 3 3.0) t)
(qa (= 0.3 (+ 0.1 0.2)) nil) ;; float rounding: 0.1+0.2 = 0.30000000000000004
(qa (< 2 2.0) nil)
(qa (<= 2 2.0) t)
(qa (> 2.0000000000000004 2) t)

;; == logcount / ash / lsh boundaries ==
(qa (logcount 0) 0)
(qa (logcount -1) 0) ;; no 0 bits inside the fixnum range
(qa (logcount 255) 8)
(qa (logcount -255) 7)
(qa (logcount 13) 3)
(qa (logcount -13) 2)
(qa (ash 1 62) 4611686018427387904)
(qa (ash -1 62) -4611686018427387904)
;; lsh is a LOGICAL shift — fills zeros, ignores sign
(qa (lsh -1 -1) 2305843009213693951) ;; → most-positive-fixnum
(qa (lsh 4 -2) 1)
(qa (lsh -4 -2) 1152921504606846975)
;; ash is arithmetic (sign-preserving)
(qa (ash -4 -1) -2)
(qa (ash -4 -2) -1)
(qa (ash 7 -1) 3)

;; == cl-gcd / cl-lcm ==
(qa (cl-gcd 12 18) 6)
(qa (cl-gcd 0 5) 5)
(qa (cl-gcd 5 0) 5)
(qa (cl-gcd -12 18) 6)
(qa (cl-gcd 0 0) 0)
(qa (cl-lcm 4 6) 12)
(qa (cl-lcm 0 5) 0)
(qa (cl-lcm -4 6) 12)

;; == isqrt does not exist in 31.1 ==
(qa (fboundp 'isqrt) nil)
;; integer sqrt via sqrt+floor
(qa (floor (sqrt 16)) 4)
(qa (floor (sqrt 15)) 3)

;; == cl-make-random-state seeded determinism ==
;; (cl-make-random-state INT) seeds; (cl-random LIM STATE) consumes STATE.
;; GNU pins: seed 42 → 51 82 73; seed 0 → 57 86.
(qa (let ((s (cl-make-random-state 42)))
      (list (cl-random 100 s) (cl-random 100 s) (cl-random 100 s)))
    '(51 82 73))
(qa (let ((s (cl-make-random-state 0)))
      (list (cl-random 100 s) (cl-random 100 s)))
    '(57 86))
;; two same-seed states produce identical streams
(qa (let ((s1 (cl-make-random-state 42)) (s2 (cl-make-random-state 42)))
      (equal (list (cl-random 100 s1) (cl-random 100 s1))
             (list (cl-random 100 s2) (cl-random 100 s2))))
    t)
;; different seeds diverge (probabilistically, but pinned for these seeds)
(qa (let ((s1 (cl-make-random-state 1)) (s2 (cl-make-random-state 2)))
      (equal (cl-random 1000 s1) (cl-random 1000 s2)))
    nil)
;; float LIM: result in [0, lim), pinned value
(qa (let ((s (cl-make-random-state 7)))
      (cl-random 10.0 s))
    9.923129081726074)
(qa (let ((s (cl-make-random-state 7))) (floatp (cl-random 10.0 s))) t)
;; cl-random-state-p predicate
(qa (let ((s (cl-make-random-state 1))) (cl-random-state-p s)) t)
(qa (cl-random-state-p 42) nil)
;; snapshot/replay: copy-sequence does NOT deep-copy the state record
;; (shared vec) — GNU quirk; replay needs i/j/vec fields copied by hand
(qa (let* ((s (cl-make-random-state 42))
           (_ (cl-random 100 s)))
      (let* ((i (cl--random-state-i s)) (j (cl--random-state-j s))
             (v (cl-copy-seq (cl--random-state-vec s)))
             (saved (cl--make-random-state 0))
             (a (cl-random 100 s)) (b (cl-random 100 s)))
        (aset saved 1 i) (aset saved 2 j) (aset saved 3 v)
        (equal (list a b) (list (cl-random 100 saved) (cl-random 100 saved)))))
    t)
;; cl-random without STATE uses the global state; results are still bounded
(qa (>= (cl-random 10) 0) t)
(qa (< (cl-random 10) 10) t)
(qa (floatp (cl-random 10.0)) t)


;; == copysign sign rules ==
(qa (copysign 3.0 -1.5) -3.0)
(qa (copysign -3.0 1.0) 3.0)
(qa (copysign 0.0 -1.0) -0.0)

;; == isnan / float-equality pitfalls ==
(qa (isnan 0.0e+NaN) t)
(qa (isnan 1.0) nil)
(qa (isnan (/ 0.0 0.0)) t)
;; NaN is never equal to anything, even itself
(qa (= 0.0e+NaN 0.0e+NaN) nil)
(qa (= 1.0e+INF 1.0e+INF) t)
(qa (< 1.0e+INF -1.0e+INF) nil)

(princ (qa--done))
