;; cl-loop — GNU 31.1 pins (verified): in/on/across iteration, accumulation
;; clauses, conditionals with `it', with/initially/finally, named, into.
;; NOTE: `qa' evals FORMs dynamically — `let' must be INSIDE each form.
;;
;; GNU truths pinned:
;;   - `for x in LIST by FN' steps with FN (cddr skips).
;;   - `for x on LIST' iterates tails.
;;   - `for x across VEC/STRING' iterates elements (chars of a string).
;;   - `repeat N' iterates N times.
;;   - `for i = INIT then STEP' is a general stepping clause.
;;   - parallel `for' clauses stop at the shortest sequence.
;;   - `if/when/unless ... else' with `it' bound to the test value.
;;   - `collect ... into VAR' keeps FORWARD order; `finally return' needed
;;     to expose accumulators (the loop's value is otherwise nil).
;;   - `append EXPR into b' appends a FRESH '(x y) each iteration —
;;     GNU cl-loop does not copy: yields (x y x y ...) shares? — it
;;     yields the literal each time, so 3 iterations give (x y x y x y).
;;   - `thereis'/`always'/`never' return t/nil.
;;   - `named' + cl-return-from breaks out of nested loops.
;;   - `with x = N' binds for the whole body.
;;   - hash iteration: `being the hash-keys/hash-values of TABLE'.
;;   - `concat' accumulates strings.
;;   - avoid: `(cl-loop)' empty body and `with a = 1 and b = 2' —
;;     GNU's expander hangs on them (verified — do not pin).
;;   - remacs prelude provides cl-loop (fboundp is nil but macroexpand works).
;; remacs detection: GNU emacs-version is "31.1"; remacs's says "remacs".
(defvar qa--remacs
  (and (boundp 'emacs-version) (stringp emacs-version)
       (string-match "remacs" emacs-version)))
;; GNU: cl-loop is autoloaded only via cl-lib; remacs's prelude defines
;; it, and `(require 'cl-lib)' completes now — require on both.
(require 'cl-lib)

;; == for x in / on / across ==
(qa (cl-loop for x in '(a b c) collect x) '(a b c))
(qa (cl-loop for x in '(a b c) by #'cddr collect x) '(a c))
(qa (cl-loop for x on '(a b c) collect x) '((a b c) (b c) (c)))
(qa (cl-loop for x on '(a b c d) by #'cddr collect x) '((a b c d) (c d)))
(qa (cl-loop for x across [10 20 30] collect x) '(10 20 30))
(qa (cl-loop for x across "ab" collect x) '(97 98))

;; == below / downfrom+above / repeat ==
(qa (cl-loop for i below 4 collect i) '(0 1 2 3))
(qa (cl-loop for i below 0 collect i) nil)
(qa (cl-loop for i downfrom 3 above 0 collect i) '(3 2 1))
(qa (cl-loop repeat 3 collect 'x) '(x x x))

;; == for VAR = INIT then STEP ==
(qa (cl-loop for i = 0 then (1+ i) while (< i 4) collect i) '(0 1 2 3))
(qa (cl-loop for i = 5 then (* i 2) repeat 3 collect i) '(5 10 20))
(qa (cl-loop for i below 3 for x = (* i 10) collect x) '(0 10 20))

;; == parallel fors stop at the shortest ==
(qa (cl-loop for i in '(1 2 3) for j in '(a b) collect (list i j))
    '((1 a) (2 b)))
(qa (cl-loop for x in '(a b) for i below 5 collect (list x i))
    '((a 0) (b 1)))

;; == destructuring in for ==
(qa (cl-loop for (a b) in '((1 x) (2 y)) collect (list a b))
    '((1 x) (2 y)))

;; == accumulation clauses ==
(qa (cl-loop for i in '(1 2 3 4) append (list i i)) '(1 1 2 2 3 3 4 4))
(qa (cl-loop for i in '(1 2 3) nconc (list i (- i))) '(1 -1 2 -2 3 -3))
(qa (cl-loop for i in '(1 2 3 4) count (cl-evenp i)) 2)
(qa (cl-loop for i in '(1 2 3 4) sum i) 10)
(qa (cl-loop for i in '(2 4 6) sum (* i 2)) 24)
(qa (cl-loop for i in '(3 1 2) minimize i) 1)
(qa (cl-loop for i in '(3 1 2) maximize i) 3)
(qa (cl-loop for i in nil minimize i) nil)
(qa (cl-loop for i from 1 to 3 concat (number-to-string i)) "123")

;; == do clause returns nil; side effects still run ==
(qa (cl-loop for i below 3 do (princ "")) nil)
(qa (let ((acc nil))
      (cl-loop for i in '(1 2 3) do (push i acc))
      acc)
    '(3 2 1))

;; == with binds for the whole loop ==
(qa (cl-loop with x = 10 for i below 3 collect (+ i x)) '(10 11 12))

;; == finally return / finally side effects ==
(qa (cl-loop for i below 3 collect i finally return 'done) 'done)
(qa (cl-loop for i below 3 finally return i) 3)
(qa (let ((acc nil))
      (cl-loop for i in '(1 2 3)
               initially (setq acc 'start)
               do (push i acc)
               finally (push 'f acc))
      acc)
    '(f 3 2 1 . start))
;; initially runs BEFORE the body even when the loop does zero iterations
(qa (let ((acc nil))
      (cl-loop for i in nil initially (setq acc 'ran) do (push i acc))
      acc)
    'ran)

;; == if / when / unless + it + else ==
(qa (cl-loop for i in '(1 2 3 4) if (cl-evenp i) collect i) '(2 4))
(qa (cl-loop for i in '(1 2 3 4) if (cl-evenp i) collect i else collect (- i))
    '(-1 2 -3 4))
(qa (cl-loop for x in '(1 nil 3) when x collect it) '(1 3))
(qa (cl-loop for x in '(1 nil 3) when x collect it else collect 'no)
       '(1 no 3))
(qa (cl-loop for x in '(1 nil 3) unless x collect 'nil-seen) '(nil-seen))
(qa (cl-loop for i in '(1 2 3 4) when (cl-evenp i) sum i) 6)

;; == while / until ==
(qa (cl-loop for i below 10 while (< i 3) collect i) '(0 1 2))
(qa (cl-loop for i below 10 until (> i 2) collect i) '(0 1 2))

;; == thereis / always / never ==
(qa (cl-loop for i below 5 thereis (= i 3)) t)
(qa (cl-loop for i below 5 always (< i 10)) t)
(qa (cl-loop for i below 5 never (> i 4)) t)
(qa (cl-loop for i below 5 thereis (= i 9)) nil)
(qa (cl-loop for i below 5 always (< i 3)) nil)

;; == into + and ==
(qa (cl-loop for i in '(1 2 3) collect i into xs finally return (reverse xs))
    '(3 2 1))
(qa (cl-loop for i below 3
             collect i into xs
             and collect (* i i) into ys
             finally return (list xs ys))
    '((0 1 2) (0 1 4)))
(qa (cl-loop for i below 3 sum i into s finally return s) 3)
(qa (cl-loop for i in '(1 2 3)
             collect i into a
             append '(x y) into b
             finally return (list a b))
    '((1 2 3) (x y x y x y)))

;; == return / named + cl-return-from ==
(qa (cl-loop for i below 3 return i) 0)
(qa (cl-loop for i below 3 when (= i 1) return 'found) 'found)
(qa (cl-loop named outer
             for i below 3
             do (cl-loop named inner
                         for j below 3
                         when (= j 1) do (cl-return-from outer 'bail)))
    'bail)
(qa (cl-loop return 5) 5)

;; == being the hash-keys / hash-values ==
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h) (puthash 'b 2 h)
      (sort (cl-loop for k being the hash-keys of h collect k) #'string<))
    '(a b))
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h) (puthash 'b 2 h)
      (sort (cl-loop for v being the hash-values of h collect v) #'<))
    '(1 2))
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h) (puthash 'b 2 h)
      (length (cl-loop for k being the hash-keys of h
                       using (hash-value v)
                       collect (list k v))))
    2)

;; == being the buffers ==
(qa (cl-loop for buf being the buffers
             when (string-match "scratch" (buffer-name buf))
             return (buffer-name buf))
    "*scratch*")

;; == multiple accumulation clauses in one iteration ==
(qa (cl-loop for i in '(1 2) collect i collect (* i 10)) '(1 10 2 20))

(princ (qa--done))
