;; cl-loop — GNU 31.1 pins: `it' binds the TEST value in if/when/unless
;; clauses (even on the else side); `until' tests AFTER the body; bare
;; `loop' is NOT defined in 31.1; mixed anonymous collect+sum errors.
;; remacs: cl-loop comes from prelude.el.
(require 'cl-lib)

;; == repeat / while / until ==
(qa (cl-loop repeat 3 collect 'x) '(x x x))
(qa (cl-loop repeat 0 collect 'x) nil)
(qa (cl-loop repeat 3 return 'x) 'x)
(qa (cl-loop repeat 2 for i in '(a b c) collect i) '(a b))
(qa (cl-loop for i in '(a b c) repeat 2 collect i) '(a b))
(qa (cl-loop for i from 1 while (< i 4) collect i) '(1 2 3))
(qa (cl-loop for i from 1 until (> i 3) collect i) '(1 2 3))
(qa (cl-loop for i from 1 while nil collect i) nil)
;; while/until AFTER collect: the body already ran for that iteration
(qa (cl-loop for i from 1 collect i while (< i 3)) '(1 2 3))
(qa (cl-loop for i from 1 collect i until (> i 3)) '(1 2 3 4))
(qa (cl-loop for i below 4 until (= i 2) collect i) '(0 1))
(qa (cl-loop for i below 4 while (/= i 2) collect i) '(0 1))

;; == always / never / thereis ==
(qa (cl-loop for i in '(2 4 6) always (cl-evenp i)) t)
(qa (cl-loop for i in '(2 3 6) always (cl-evenp i)) nil)
(qa (cl-loop for i in nil always (cl-evenp i)) t)
(qa (cl-loop for i in '(2 4 6) always i) t) ;; normalizes to t, not last value
(qa (cl-loop for i in '(1 3) never (cl-evenp i)) t)
(qa (cl-loop for i in '(1 4) never (cl-evenp i)) nil)
(qa (cl-loop for i in nil never t) t)
(qa (cl-loop for i in '(a 1 b) thereis (and (numberp i) (* i 10))) 10)
(qa (cl-loop for i in '(a b) thereis (numberp i)) nil)
(qa (cl-loop for i in nil thereis i) nil)
(qa (cl-loop for i from 1 thereis (when (> i 5) i)) 6)
(qa (cl-loop for i in '(1 2) thereis i) 1) ;; returns the non-nil value
;; `thereis'/`always' early-exit: thereis does not bind `it'
(qa-err (cl-loop for i in '(1 2 3) thereis (and (cl-oddp i) it))
        'void-variable)
;; always/thereis/never exit the loop; finally return still wins
(qa (cl-loop for i in '(2 4) always (cl-evenp i) finally return 'done) 'done)
(qa (cl-loop for i in '(a b) thereis (numberp i) finally return 'done) 'done)

;; == accumulation verbs ==
(qa (cl-loop for i in '(1 2 3) collect (* i i)) '(1 4 9))
(qa (cl-loop for i in '(1 2 3) collecting i) '(1 2 3))
(qa (cl-loop for i in '((a . 1) (b . 2)) collect (car i)) '(a b))
(qa (cl-loop for i in '((1) (2 3) nil (4)) append i) '(1 2 3 4))
(qa (cl-loop for i in '((1) (2 3)) nconc (copy-sequence i)) '(1 2 3))
(qa (cl-loop for i in '((1) (2)) appending i) '(1 2))
(qa (cl-loop for x in nil append x) nil)
(qa (cl-loop for s in '("a" "b" "c") concat s) "abc")
(qa (cl-loop for s in '("x" "y") concating s) "xy")
(qa (cl-loop for s in '("a" "b") concat (concat s "-")) "a-b-")
(qa (cl-loop for x in nil concat x) "")
(qa (cl-loop for x in '([1 2] [3]) vconcat x) [1 2 3])
(qa (cl-loop for x in '([1]) vconcating x) [1])
(qa (cl-loop for x in nil vconcat x) [])
(qa (cl-loop for i in '(1 2 3) sum i) 6)
(qa (cl-loop for i in '(1 2 3) summing i) 6)
(qa (cl-loop for i in '(1 2 3) sum (* i i)) 14)
(qa (cl-loop for i in nil sum i) 0)
(qa (cl-loop for i in '(1 2 3) count (cl-oddp i)) 2)
(qa (cl-loop for i in '(1 2 3) counting (cl-oddp i)) 2)
(qa (cl-loop for i in '(1 2 3) counting i) 3)
(qa (cl-loop for i in nil count t) 0)
(qa (cl-loop for i in '(3 1 4 1 5) minimize i) 1)
(qa (cl-loop for i in '(3 1 4 1 5) maximize i) 5)
(qa (cl-loop for i in '(3 1) minimizing i) 1)
(qa (cl-loop for i in '(3 1) maximizing i) 3)
(qa (cl-loop for i in nil minimize i) nil) ;; empty: nil, not error
(qa (cl-loop for i in nil maximize i) nil)
;; GNU quirk: two DIFFERENT anonymous accumulators collide at runtime
(qa-err (cl-loop for i in '(1 2 3) collect i sum i) 'wrong-type-argument)
(qa-err (cl-loop for i in '(1 2 3) sum i collect i) 'wrong-type-argument)
;; …but named accumulators of different types coexist fine
(qa (cl-loop for i below 3 collect (* i i) into squares
             sum (* i i) into total
             finally return (list squares total))
    '((0 1 4) 5))
;; running accumulator visible mid-loop
(qa (cl-loop for i in '(1 2 3) sum i into s collect (list i s))
    '((1 1) (2 3) (3 6)))

;; == if / when / unless / else / end / it ==
(qa (cl-loop for i in '(1 2 3 4)
             if (cl-oddp i) collect i into odds
             else collect i into evens
             finally return (list odds evens))
    '((1 3) (2 4)))
(princ (qa--done))
