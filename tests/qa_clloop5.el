;; cl-loop / cl-macs iteration — GNU 31.1 pins (continued): `and'-joined
;; `for'/`with' clauses bind PARALLEL (a `let'), so `with a = 1 and b = a'
;; is a void-variable unless an outer a exists; early exits (`return',
;; `cl-return', a failing `always'/`never'/`thereis') SKIP `finally' and
;; return the result-var; a named loop turns `finally do (return X)' into
;; a real (return X) call → void-function; `unless … collect it' collects
;; the TEST value, so the else side collects nil/t not the item; an `if'
;; clause without `end' swallows the next clause as its else; a `cl-loop'
;; with no clause list expands to `(while t)' (infinite loop — checked via
;; macroexpand only); in-ref/across-ref/of-ref expose setf-able places via
;; cl-symbol-macrolet; `dolist'/`cl-dolist' result forms see the var as
;; VOID in 31.1 (the var is bound only inside the body) while `dotimes'
;; leaves it bound.
;; remacs: cl-loop comes from prelude.el.
(require 'cl-lib)

;; == `with' — sequential `with … with' vs parallel `with … and' ==
(qa (eval '(cl-loop with a = 1 with b = (1+ a) return (list a b)) t) '(1 2))
(qa (let ((a 99)) (cl-loop with a = 1 and b = a return (list a b))) '(1 99))
(qa-err (eval '(cl-loop with a = 1 and b = a return (list a b)) (list (cons 'a a))) 'void-variable)
(qa (eval '(cl-loop with a return a) t) nil) ;; `with VAR' alone binds nil
(qa (eval '(cl-loop with (x y) = '(1 2) return (list x y)) t) '(1 2))
(qa (eval '(cl-loop with (x . y) = '(1 . 2) return (list x y)) t) '(1 2))
;; first clause parsed becomes the OUTER binding
(qa (eval '(cl-loop with w = 3 for i below w collect i) t) '(0 1 2))
(qa-err (eval '(cl-loop for i below w with w = 3 collect i) t) 'void-variable)

;; == `for … and …' — parallel multi-var iteration ==
(qa (eval '(cl-loop for x in '(a b c) and y in '(1 2) collect (list x y)) t)
    '((a 1) (b 2))) ;; stops at the shortest
(qa (eval '(cl-loop for i from 0 and x in '(a b c) collect (list i x)) t)
    '((0 a) (1 b) (2 c)))
(qa (eval '(cl-loop for x in '(a b c) and i from 10 collect (list x i)) t)
    '((a 10) (b 11) (c 12)))
(qa (eval '(cl-loop for i below 3 and j below 10 collect (list i j)) t)
    '((0 0) (1 1) (2 2)))
(qa (eval '(cl-loop for x on '(1 2 3) and i below 2 collect (list i x)) t)
    '((0 (1 2 3)) (1 (2 3))))
;; `=' inside an `and' group: start re-binds each iteration, `then' steps
;; via psetq gated on the group conditions
(qa (eval '(cl-loop for x = 0 and y = 1 then (+ x y) repeat 4 collect (list x y)) t)
    '((0 1) (0 1) (0 1) (0 1)))
(qa (eval '(cl-loop for x = 0 then (1+ x) and y = 10 then (* y 2)
             repeat 3 collect (list x y)) t)
    '((0 10) (1 20) (2 40)))
(qa (eval '(cl-loop for x = 0 then (+ x 2) and y in '(a b c) collect (list x y)) t)
    '((0 a) (2 b) (4 c)))
(qa (eval '(cl-loop for x in '(a b c) and i = 0 then (1+ i) collect (list x i)) t)
    '((a 0) (b 1) (c 2)))
;; a separate `for' clause is NOT in the `and' group — its condition does
;; not gate the stepped var
(qa (eval '(cl-loop for x in '(a b) and i = 0 then (1+ i)
             for j below 10 collect (list x i j)) t)
    '((a 0 0) (b 1 1)))

;; == `for VAR = EXPR [then EXPR2]' extras ==
(qa (eval '(cl-loop for x = 1 then (* x 2) until (>= x 20) collect x) t) '(1 2 4 8 16))
(qa (eval '(cl-loop for x = 0 then (1+ x) until (>= x 3) collect x) t) '(0 1 2))
;; no `then': the start expr is re-evaluated EVERY iteration
(qa (let ((n 0)) (cl-loop for x = (cl-incf n) repeat 3 collect x)) '(1 2 3))
(qa (let ((n 0)) (cl-loop for x = (cl-incf n) then 'z repeat 3 collect x))
    '(1 z z))
(qa (eval '(cl-loop for x = 'a for i below 2 collect x) t) '(a a))
;; `as' is a `for' synonym
(qa (eval '(cl-loop as x in '(1 2 3) collect (* x x)) t) '(1 4 9))

;; == destructuring for-vars ==
(qa (eval '(cl-loop for (a b) in '((1 2) (3 4)) collect (+ a b)) t) '(3 7))
(qa (eval '(cl-loop for (a . b) in '((1 . 2) (3 . 4)) collect (list a b)) t)
    '((1 2) (3 4)))
(qa (eval '(cl-loop for (a (b c)) in '((1 (2 3)) (4 (5 6))) collect (list a b c)) t)
    '((1 2 3) (4 5 6)))
(qa (eval '(cl-loop for (a b) on '((1 2) (3 4)) collect a) t) '((1 2) (3 4)))
;; …but `across' does not destructure (aref element applied to pop/car)
(qa-err (eval '(cl-loop for (a b) across [[1 2] [3 4]] collect (+ a b)) t)
        'wrong-type-argument)

;; == in-ref / across-ref / of-ref — setf-able iteration ==
(qa (let ((l (list 1 2 3)))
      (cl-loop for x in-ref l do (cl-incf x 10)) l)
    '(11 12 13))
(qa (let ((l (list 1 2 3))) (cl-loop for x in-ref l collect x)) '(1 2 3))
(defvar qad-v nil)
(setq qad-v (vector 1 2 3))
(qa (progn (eval '(cl-loop for x across-ref qad-v do (setq x (* x x)))) qad-v)
    [1 4 9])
(defvar qad-v nil)
(setq qad-v [1 2 3])
(qa (eval '(cl-loop for x across-ref qad-v collect x)) '(1 2 3))
(defvar qad-s nil)
(setq qad-s (copy-sequence "abc"))
(qa (progn (eval '(cl-loop for c across-ref qad-s do (setq c (upcase c)))) qad-s)
    "ABC")
(defvar qad-v2 nil)
(setq qad-v2 (vector 1 2))
(qa (progn (eval '(cl-loop for x being the elements of-ref qad-v2 do (cl-incf x))) qad-v2)
    [2 3])
(qa (let ((l (list 1 2)))
      (cl-loop for x being the elements in-ref l do (cl-incf x 5)) l)
    '(6 7))
(qa (eval '(cl-loop for x being the elements in-ref (list 1 2 3) collect x) t)
    '(1 2 3))

;; == being the elements … using (index VAR) ==
(qa (eval '(cl-loop for x being the elements of '(a b c) using (index i)
             collect (list x i)) t)
    '((a 0) (b 1) (c 2)))
(qa (eval '(cl-loop for x being the elements of [10 20] using (index i)
             collect (list x i)) t)
    '((10 0) (20 1)))
(qa (eval '(cl-loop for x being the elements of "hi" using (index i)
             collect (list x i)) t)
    '((104 0) (105 1)))
(qa (eval '(cl-loop for x being the elements of "" collect x) t) nil)
(qa (eval '(cl-loop for x being the elements of nil collect x) t) nil)

;; == being the symbols ==
(defvar qad-ob nil)
(setq qad-ob (make-vector 7 0))
(intern "zz" qad-ob) (intern "aa" qad-ob)
(qa (sort (eval '(cl-loop for s being the symbols of qad-ob collect (symbol-name s)))
          #'string<)
    '("aa" "zz"))
(qa (eval '(cl-loop for s being the symbols of (make-vector 7 0) collect s) t) nil)
(qa (eval '(cl-loop for s being the symbols thereis (eq s 'cl-loop)) t) t)
(qa (eval '(cl-loop for s being each symbol of obarray thereis (eq s 'car)) t) t)
;; present-/external-symbols iterate the same obarray in Emacs Lisp
(defvar qad-ob nil)
(setq qad-ob (make-vector 7 0))
(intern "a1" qad-ob)
(qa (eval '(cl-loop for s being the external-symbols of qad-ob collect
             (symbol-name s)))
    '("a1"))
(defvar qad-ob nil)
(setq qad-ob (make-vector 7 0))
(intern "a1" qad-ob)
(qa (eval '(cl-loop for s being the present-symbols of qad-ob collect
             (symbol-name s)))
    '("a1"))

;; == being the buffers / frames / windows ==
(qa (eval '(cl-loop for b being the buffers count t) t) (length (buffer-list)))
(qa (eq (eval '(cl-loop for b being the buffers return b) t) (current-buffer)) t)
(qa (eval '(cl-loop for b being the buffers thereis (eq b (current-buffer))) t) t)
(qa (eval '(cl-loop for f being the frames count t) t) (length (frame-list)))
(qa (eval '(cl-loop for f being the screens count t) t) (length (frame-list)))
(qa (eq (eval '(cl-loop for f being the frames return f) t) (selected-frame)) t)
(qa (eval '(cl-loop for f being the frames collect (framep f)) t) '(t))
(qa (eval '(cl-loop for w being the windows count t) t) (length (window-list)))
(qa (eq (eval '(cl-loop for w being the windows return w) t) (selected-window)) t)
(qa (eval '(cl-loop for w being the windows of (selected-frame) count t) t) 1)
;; `buffers' takes no `of' argument
(qa-err (eval '(eval '(cl-loop for b being the buffers of nil collect b)) t) 'error)

;; == being the key-codes / key-bindings / key-seqs ==
(defvar qad-m nil)
(setq qad-m (make-sparse-keymap))
(define-key qad-m "a" 'cmd-a) (define-key qad-m "b" 'cmd-b)
(qa (sort (eval '(cl-loop for k being the key-codes of qad-m collect k)) #'<)
    '(97 98))
(defvar qad-m nil)
(setq qad-m (make-sparse-keymap))
(define-key qad-m "a" 'cmd-a) (define-key qad-m "b" 'cmd-b)
(qa (cl-sort (eval '(cl-loop for k being the key-bindings of qad-m collect k))
             #'string-lessp :key #'symbol-name)
    '(cmd-a cmd-b))
(defvar qad-m nil)
(setq qad-m (make-sparse-keymap))
(define-key qad-m "a" 'cmd-a)
(qa (eval '(cl-loop for k being the key-codes of qad-m using (key-bindings b)
             collect (list k b)))
    '((97 cmd-a)))
(defvar qad-m nil)
(setq qad-m (make-sparse-keymap))
(define-key qad-m "a" 'cmd-a)
(qa (eval '(cl-loop for b being the key-bindings of qad-m using (key-codes k)
             collect (list b k)))
    '((cmd-a 97)))
;; key-seqs descends into prefix maps, yielding key-sequence vectors
(defvar qad-m nil)
(setq qad-m (make-sparse-keymap))
(define-key qad-m "a" 'cmd-a) (define-key qad-m "xb" 'cmd-b)
(qa (sort (eval '(cl-loop for s being the key-seqs of qad-m collect s))
          (lambda (x y) (< (aref x 0) (aref y 0))))
    '([97] [120 98]))
;; key-codes only sees the top level: the prefix key counts, not "b"
(defvar qad-m nil)
(setq qad-m (make-sparse-keymap))
(define-key qad-m "a" 'cmd-a) (define-key qad-m "xb" 'cmd-xb)
(qa (eval '(cl-loop for k being the key-codes of qad-m count t))
    2)
(qa (eval '(cl-loop for k being the key-codes of (make-sparse-keymap) collect k) t)
    nil)
;; `using' requires a different key-type
(qa-err (eval '(eval '(cl-loop for k being the key-codes of (make-sparse-keymap)
                        using (hash-values v) collect k)) t)
        'error)

;; == being the overlays / intervals ==
(qa (with-temp-buffer
      (insert "hello")
      (make-overlay 1 3) (make-overlay 3 6)
      (eval '(cl-loop for ov being the overlays count t) t))
    2)
(qa (with-temp-buffer
      (insert "hello")
      (make-overlay 1 3) (make-overlay 3 6)
      (sort (eval '(cl-loop for ov being the overlays collect (overlay-start ov)) t)
            #'<))
    '(1 3))
(qa (with-temp-buffer
      (insert "0123456789")
      (make-overlay 1 2) (make-overlay 5 6) (make-overlay 8 9)
      (eval '(cl-loop for ov being the overlays from 4 to 7
               collect (overlay-start ov)) t))
    '(5))
;; intervals cover the WHOLE range — runs without the property included
(qa (with-temp-buffer
      (insert "hello")
      (put-text-property 1 3 'face 'bold)
      (put-text-property 4 6 'face 'italic)
      (eval '(cl-loop for iv being the intervals property 'face collect iv) t))
    '((1 . 3) (3 . 4) (4 . 6)))
(qa (with-temp-buffer
      (insert "hi")
      (put-text-property 1 2 'face 'bold)
      (eval '(cl-loop for (s . e) being the intervals property 'face count t) t))
    2)
(defvar qad-s nil)
(setq qad-s (copy-sequence "hello"))
(put-text-property 0 2 'face 'bold qad-s)
(qa (eval '(cl-loop for iv being the intervals of qad-s property 'face collect iv))
    '((0 . 2) (2 . 5)))
;; list-destructuring needs the dotted form: (s e) errs on cons intervals
(qa-err (with-temp-buffer
          (insert "hello")
          (eval '(cl-loop for (s e) being the intervals collect (list s e)) t))
        'wrong-type-argument)

;; == do / doing ==
(qa (let ((n 0)) (cl-loop for i below 5 do (setq n (+ n i))) n) 10)
(qa (let ((n 0)) (cl-loop repeat 3 doing (setq n (1+ n)) (setq n (* n 2)))
      n)
    14)
(qa (eval '(cl-loop for i below 3 do (list 1 2) collect i) t) '(0 1 2))
;; `do' consumes every following form until the next clause word
(qa (let ((n 0)) (cl-loop for i below 3 doing (cl-incf n) (cl-incf n 10))
      n)
    33)
(qa (let ((a 0) (b 0))
      (cl-loop for i below 2 doing (setq a i) (setq b (* i 10)))
      (list a b))
    '(1 10))
;; `do' with no form at all: syntax error at expansion
(qa-err (eval '(eval '(cl-loop for i below 2 do)) t) 'error)

;; == return / named ==
(qa (eval '(cl-loop for i below 10 return 'stop) t) 'stop)
(qa (eval '(cl-loop for i below 10 when (> i 2) return i) t) 3)
(qa (eval '(cl-loop for i in '(a 1 b) when (numberp i) return (* i 10)) t) 10)
(qa (eval '(cl-loop named q for i below 9 when (> i 2) return i) t) 3)
;; `it' is the test VALUE inside if/when — `return it' returns it
(qa (eval '(cl-loop for i in '(1 2 3) if (> i 1) return it) t) t)
(qa (eval '(cl-loop for i in '(1 2 3) if (> i 1) return it else return 'no) t) 'no)
(qa (eval '(cl-loop for i in '(1 2) when (cl-oddp i) return it) t) t)
;; `cl-return' exits the implicit nil block — skipping `finally'
(qa (eval '(cl-loop for i below 5 when (= i 2) do (cl-return 'early)) t) 'early)
(qa (let ((n 0))
      (list (cl-loop for i below 5 when (= i 2) do (cl-return 'early)
                     finally do (setq n 9))
            n))
    '(early 0))
;; named loops support cl-return-from …
(qa (eval '(cl-loop named outer for i below 5 when (= i 3)
             do (cl-return-from outer 'out)) t)
    'out)
(qa (eval '(cl-loop named n for i below 3 when (= i 1)
             do (cl-return-from n 'gone) collect i) t)
    'gone)
(qa (eval '(cl-loop named outer for i below 3 collect i) t) '(0 1 2))
;; …and make bare `cl-return' unbound (no nil block is established)
(qa-err (eval '(cl-loop named n for i below 3 do (cl-return 'x)) t) 'void-variable)
(qa-err (eval '(cl-loop named n1 for i below 3 do (cl-return-from n2 'x)) t)
        'void-variable)

;; == initially / finally ==
(qa (let ((log nil))
      (cl-loop initially (push 'a log)
               for i below 2 do (push i log)
               finally (push 'z log)
               return log))
    '(0 a)) ;; finally skipped after `return' → log is (0 a)
(qa (eval '(cl-loop initially do (ignore) for i below 3 sum i) t) 3)
(qa (let ((n 0))
      (cl-loop initially do (setq n 5) (setq n (1+ n))
               for i below 1 return n))
    6)
(qa (eval '(cl-loop for i below 3 finally return 'done) t) 'done)
(qa (eval '(cl-loop for i below 3 collect i finally return 'done) t) 'done)
(qa (eval '(cl-loop for i below 3 finally do (ignore 'x) return 99) t) 99)
(qa (eval '(cl-loop for i below 3 finally return (list 'a i)) t) '(a 3))
;; `finally do (return X)' on an unnamed loop = `finally return' …
(qa (eval '(cl-loop for i below 3 finally do (return 99)) t) 99)
;; …but inside a NAMED loop it is a literal call to `return' → void
(qa-err (eval '(cl-loop named n for i below 3 finally do (return 99)) t)
        'void-function)
(qa (eval '(cl-loop named n for i below 3 finally return 5) t) 5)
;; finally is SKIPPED when the loop exits early (flag already nil)
(qa (let ((n 0))
      (list (cl-loop for i below 5 when (= i 2) return 'hit
                     finally do (setq n 9))
            n))
    '(hit 0))
(qa (let ((n 0))
      (list (cl-loop for i in '(2 3) always (cl-evenp i)
                     finally do (setq n 1))
            n))
    '(nil 0))
(qa (eval '(cl-loop for i in '(2 3) always (cl-evenp i) finally return 'done) t) nil)
(qa (eval '(cl-loop for i in '(a 1) thereis (numberp i) finally return 'done) t) t)
(qa (eval '(cl-loop for i in '(a b) never (numberp i) finally return 'done) t) 'done)
(qa (eval '(cl-loop for i in '(1 2) never (cl-oddp i) finally return 'never-ran) t)
    nil)
(qa (eval '(cl-loop for i in '(2 4) never (cl-oddp i) finally return 'never-ran) t)
    'never-ran)
(qa (eval '(cl-loop for i in '(1) always (cl-oddp i) finally return 'ok) t) 'ok)
;; thereis falls through to finally only when nothing matched
(qa (let ((n 0))
      (list (cl-loop for i in '(a b) thereis (numberp i)
                     finally do (setq n 1))
            n))
    '(nil 1))

;; == accumulation targets and `into' ==
(qa (eval '(cl-loop for x in '((1) (2 3) nil) append x) t) '(1 2 3))
(qa (eval '(cl-loop for x in '(1 2) append (list x x)) t) '(1 1 2 2))
(qa (eval '(cl-loop for i in '((1) (2 3)) append i into acc finally return acc) t)
    '(1 2 3))
(qa (eval '(cl-loop for i in '((1 2) (3 4)) nconc (copy-sequence i)) t) '(1 2 3 4))
(qa (eval '(cl-loop for i in '((a . 1) (b . 2)) nconc (list (car i) (cdr i))) t)
    '(a 1 b 2))
(qa (eval '(cl-loop for x in '((1) (2)) nconc (copy-sequence x) into acc
             finally return acc) t)
    '(1 2))
;; anonymous nconc destructively reuses the argument cells
(qa (let ((a (list 1 2)) (b (list 3 4)))
      (list (cl-loop for x in (list a b) nconc x) a b))
    '((1 2 3 4) (1 2 3 4) (3 4)))
(qa (eval '(cl-loop for x in '([97 98] "cd") concat x) t) "abcd")
(qa (eval '(cl-loop for x in '((1 2) [3]) vconcat x) t) [1 2 3])
(qa (eval '(cl-loop for c across "abc" vconcat (vector c)) t) [97 98 99])
(qa (eval '(cl-loop for s in '("a" "b") concat s into acc finally return acc) t) "ab")
(qa (eval '(cl-loop for v in '([1] [2 3]) vconcat v into acc finally return acc) t)
    [1 2 3])
;; named accumulators: the loop result is still nil without `finally return'
(qa (eval '(cl-loop for i below 3 collect i into bag) t) nil)
(qa (eval '(cl-loop for i below 3 sum i into tot) t) nil)
(qa (eval '(cl-loop for i in '(1 2) collect i into bag collect (* i 10) into bag
             finally return bag) t)
    '(1 10 2 20))
(qa (eval '(cl-loop for i in '(3 1 4) minimize i into lo maximize i into hi
             finally return (list lo hi)) t)
    '(1 4))
(qa (eval '(cl-loop for i in nil minimize i into lo finally return lo) t) nil)
(qa (eval '(cl-loop for i in '(1 2 3 4) count (cl-evenp i) into ev
             finally return ev) t)
    2)
(qa (eval '(cl-loop for x in '(1 2 3) minimize (- x) into lo finally return lo) t) -3)
(qa (eval '(cl-loop for i below 4 count i into n sum i into s
             finally return (list n s)) t)
    '(4 6))
;; same anonymous accumulator type can be used repeatedly
(qa (eval '(cl-loop for i below 3 collect i collect (* i i)) t) '(0 0 1 1 2 4))
(qa (eval '(cl-loop for i below 3 sum i sum (* i 2)) t) 9)
(qa (eval '(cl-loop for i below 3 append (list i) collect 0) t) '(0 0 1 0 2 0))
;; …but different anonymous types still collide (already pinned for
;; collect+sum): append + concat both target the one anonymous var
(qa-err (eval '(cl-loop for i in '((1) (2)) append i concat "x") t)
        'wrong-type-argument)

;; == thereis / always / never edges ==
(qa (eval '(cl-loop for i below 3 thereis (cl-oddp i)) t) t)
(qa (eval '(cl-loop for i in '(a b) thereis (and (symbolp i) 'yes)) t) 'yes)
(qa (eval '(cl-loop for i in '(a 5 b) thereis (and (numberp i) i)) t) 5)
;; thereis exits early; a later collect never runs and the result is the
;; thereis value, not a list
(qa (eval '(cl-loop for i below 10 thereis (and (> i 4) (* i 10)) collect i) t) 50)
(qa (eval '(cl-loop for i below 5 when (= i 3) thereis (* i 10)) t) 30)
(qa (eval '(cl-loop for i in '(1 2 3) if (cl-evenp i) thereis i) t) 2)
;; always that fails discards everything, including the collected list
(qa (eval '(cl-loop for i in '(2 4 5) always (cl-evenp i) collect i) t) nil)

;; == if / when / unless / else / end / it ==
(qa (eval '(cl-loop for i in '(1 2 3) if (cl-oddp i) collect it else collect it) t)
    '(t nil t))
(qa (eval '(cl-loop for i in '(1 2 3 4) unless (cl-evenp i) collect it) t)
    '(nil nil))
(qa (eval '(cl-loop for i in '(1 2) unless (cl-evenp i) collect it
             else collect it) t)
    '(nil t))
(qa (eval '(cl-loop for i in '(1 2) unless (cl-oddp i) collect i
             else collect 'e) t)
    '(e 2))
(qa (eval '(cl-loop for i in '(1 2 3 4) when (cl-evenp i) collect i
             and collect (* i i)) t)
    '(2 4 4 16))
(qa (eval '(cl-loop for i in '(1 2 3) if (cl-oddp i) collect i
             and collect 'x else collect 'e) t)
    '(1 x e 3 x))
(qa (eval '(cl-loop for i in '(1 2 3) when (cl-oddp i) collect it
             and collect (- i)) t)
    '(t -1 t -3))
;; without `end' the next clause becomes the else branch
(qa (eval '(cl-loop for i in '(1 2) if (cl-oddp i) collect i collect (* i 10)) t)
    '(1 10 20))
;; …and `end' explicitly terminates it
(qa (eval '(cl-loop for i in '(1 2) if (cl-oddp i) collect i
             else collect (* i 10) end collect 0) t)
    '(1 0 20 0))
(qa (eval '(cl-loop for i in '(1 2) unless (cl-oddp i) collect 'u end collect 't) t)
    '(t u t))
;; `and' at top level chains clauses
(qa (eval '(cl-loop for i below 10 while (< i 3) and collect i) t) '(0 1 2))
(qa (eval '(cl-loop for i below 5 collect i and when (cl-oddp i) collect 'odd) t)
    '(0 1 odd 2 3 odd 4))
;; a clause-less loop is `(while t)' — infinite; check the expansion only
(qa (car (macroexpand '(cl-loop))) 'cl--block-wrapper)
(qa (car (nth 2 (nth 2 (nth 1 (macroexpand '(cl-loop)))))) 'while)
;; syntax errors surface at expansion → pin through eval
(qa-err (eval '(eval '(cl-loop bogus-clause)) t) 'error)
(qa-err (eval '(eval '(cl-loop for i belowxy 3 collect i)) t) 'error)

;; == misc runtime edges ==
(qa (eval '(cl-loop for i below 0) t) nil) ;; bound exhausted, no result: nil
(qa (eval '(cl-loop repeat 0) t) nil)
(qa (eval '(cl-loop for i below 2 initially (ignore)) t) nil)
(qa-err (eval '(cl-loop repeat 'x collect 1) t) 'wrong-type-argument)
(qa-err (eval '(cl-loop for i in '(1 2) if (cl-oddp i) collect i do (list it)) t)
        'void-variable) ;; `it' is unbound in `do'

;; == cl-do / cl-do* ==
(qa (eval '(cl-do ((i 0 (1+ i))) ((> i 3) i) (ignore)) t) 4)
(qa (eval '(cl-do ((i 0 (1+ i)) (j 10 (+ j i))) ((>= i 3) (list i j))) t) '(3 13))
(qa (eval '(cl-do* ((i 0 (1+ i)) (j 10 (+ j i))) ((>= i 3) (list i j))) t) '(3 16))
;; a spec without a step keeps its value
(qa (eval '(cl-do ((x 0)) ((> x 5) x) (setq x (+ x 2))) t) 6)
(qa (eval '(cl-do ((x 5) (y 1 (1+ y))) ((> y 2) x)) t) 5)
;; result forms evaluate in order; the last one wins
(qa (eval '(cl-do ((i 0 (1+ i))) ((> i 2) 'r1 'r2)) t) 'r2)
(qa (eval '(cl-do ((l '(a b c) (cdr l)) (n 0 (1+ n))) ((null l) n)) t) 3)
(qa (let ((a nil))
      (cl-do ((i 0 (1+ i))) ((> i 2) a) (push i a) (push 's a)))
    '(s 2 s 1 s 0))
;; result forms see the final (un-stepped) values; body side effects used
;; the same vars
(qa (eval '(cl-do ((i 0 (1+ i)) (n 100)) ((> i 2) (list i n))
          (setq n (+ n i))) t)
    '(3 103))
(qa (eval '(cl-do* ((i 0 (1+ i)) (n 100 (+ n i))) ((> i 2) (list i n))) t) '(3 106))
;; initially-true endtest skips the body entirely
(qa (eval '(cl-do () (t 'done)) t) 'done)
(qa (eval '(cl-do* ((x 1) (y (1+ x)) (z (1+ y))) (t (list x y z))) t) '(1 2 3))
;; `cl-do' binds in parallel: y's init can't see the new x
(qa-err (eval '(cl-do ((x 1) (y (1+ x))) (t (list x y))) t) 'void-variable)
;; implicit nil block: cl-return aborts, skipping the result forms
(qa (eval '(cl-do ((i 0 (1+ i))) (nil) (when (= i 3) (cl-return 'out))) t) 'out)
(qa (eval '(cl-do ((i 0 (1+ i))) ((> i 3)) (cl-return 'bail)) t) 'bail)

;; == cl-dolist / cl-dotimes ==
(qa (let ((acc nil)) (cl-dolist (x '(1 2 3)) (push (* x x) acc)) acc)
    '(9 4 1))
(qa (eval '(cl-dolist (x '(1 2 3) 'res)) t) 'res)
(qa (eval '(cl-dolist (x nil 'empty)) t) 'empty)
(qa (let ((n 0)) (cl-dolist (x '(1 2) n) (cl-incf n x))) 3)
(qa (eval '(cl-dolist (x '(1 2 3 4)) (when (= x 3) (cl-return 'stop))) t) 'stop)
(qa (eval '(cl-dolist (x '(1 2 3)) (when (cl-evenp x) (cl-return x))) t) 2)
;; 31.1 quirk: the result form sees the var as VOID (bound only in body)
(qa-err (eval '(cl-dolist (x '(a b c) x)) t) 'void-variable)
(qa-err (dolist (x '(a b c) x)) 'void-variable)
(qa (let ((acc nil)) (cl-dotimes (i 4) (push i acc)) (nreverse acc))
    '(0 1 2 3))
(qa (eval '(cl-dotimes (i 3 i)) t) 3) ;; dotimes leaves i bound for the result
(qa (eval '(cl-dotimes (i 0 'zero)) t) 'zero)
(qa (eval '(cl-dotimes (i -2 'neg)) t) 'neg)
(qa (eval '(cl-dotimes (i 5) (when (= i 2) (cl-return 'half))) t) 'half)
(qa (eval '(cl-dotimes (i 3 'r) (ignore i)) t) 'r)

;; == cl-do-symbols / cl-do-all-symbols ==
(defvar qad-ob nil)
(setq qad-ob (make-vector 5 0))
(intern "bb" qad-ob)
(qa (eval '(cl-do-symbols (s qad-ob 'done) (cl-return (symbol-name s))))
    "bb")
(defvar qad-ob nil)
(setq qad-ob (make-vector 5 0))
(intern "bb" qad-ob)
(qa (eval '(cl-do-all-symbols (s 'done)
      (when (eq s (intern "bb" qad-ob)) (cl-return 'found))))
    'done) ;; the custom obarray's symbol is not in the default obarray

(princ (qa--done))
