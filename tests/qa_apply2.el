;;; -*- lexical-binding: t -*-
;; apply/funcall/apply-partially/closures — GNU 31.1 -Q --batch ground
;; truth.  (qa_apply.el owns arity-error data shapes, hooks, defmacro
;; arglists, macroexpand; this file pins the rest.)
;;
;; Pinned semantics:
;;  * apply's last arg must be a listp: non-list => wrong-type-argument
;;    listp; improper tail => wrong-type-argument listp on the TAIL.
;;    Earlier args are prepended: (apply '+ 1 2 '(3 4)) = 10.
;;    (apply '+) errors — needs ≥2 args (function + last list).
;;  * funcall on nil/unbound symbol => void-function; on numbers,
;;    vectors, non-lambda lists => invalid-function.
;;  * apply-partially returns a #[128] byte-code closure; its function is
;;    NOT validated eagerly ((apply-partially 'nosuchsym 1) succeeds;
;;    funcalling it => void-function).  Needs ≥1 arg.
;;  * funcall-with-delayed-message is (TIMEOUT MESSAGE FUNCTION) — a
;;    3-arg subr; extra args => wrong-number-of-arguments (FN . 4).
;;  * Closures made under lexical eval are `interpreted-function' typed
;;    (NOT lists!): (eval '(let ((n 7)) (lambda (x) (+ x n))) t) is a
;;    vector-like #[…] object; aref 0 = arglist, aref 1 = body.
;;  * '(closure (t) (x) x) as a data literal is NOT functionp.
;;  * macrop resolves symbol function cells: (macrop 'when) => t but
;;    (macrop 'if) => nil (`if' is a special form, not a macro).
;;  * functionp on a symbol consults fboundp — 'car t, unknown nil.

;; == apply ==
(qa (apply #'+ '(1 2 3)) 6)
(qa (apply '+ '(1 2 3)) 6)
(qa (apply #'+ 1 2 '(3)) 6)
(qa (apply '+ 1 2 '(3 4)) 10)
(qa (apply '+ nil) 0)
(qa (apply '+ 5 nil) 5)
(qa (apply 'car '((a b))) 'a)
(qa (apply 'cons '(a b)) '(a . b))
;; last arg must be a list — data is (listp BAD)
(qa (condition-case e (apply '+ 5) (error (list (car e) (cdr e))))
    '(wrong-type-argument (listp 5)))
;; improper tail: error names the tail, not the whole list
(qa (condition-case e (apply '+ '(1 . 2))
      (error (list (car e) (cdr e))))
    '(wrong-type-argument (listp 2)))
;; (apply '+) with no last arg: listp check fires first
(qa (condition-case e (apply '+) (error (list (car e) (cdr e))))
    '(wrong-type-argument (listp +)))
(qa-err (apply) 'wrong-number-of-arguments)

;; == funcall ==
(qa (funcall #'+ 1 2) 3)
(qa (funcall '+) 0)
(qa (funcall '+ 1) 1)
(qa (funcall 'car '(a)) 'a)
(qa (funcall 'list) nil)
;; remacs: nil signals invalid-function (GNU: void-function)
(qa (condition-case e (funcall nil) (error (car e))) 'void-function)
(qa (condition-case e (funcall 'qa-nosuchsym) (error (car e)))
    'void-function)
(qa (condition-case e (funcall 5) (error (car e))) 'invalid-function)
(qa (condition-case e (funcall [1 2]) (error (car e))) 'invalid-function)
(qa (condition-case e (funcall '(1 2)) (error (car e)))
    'invalid-function)
;; remacs: nil signals invalid-function (GNU: void-function)
(qa (condition-case e (apply nil '(1)) (error (car e))) 'void-function)
;; strings are not functions
(qa (condition-case e (funcall "f") (error (car e))) 'invalid-function)
;; funcall is a subr; needs ≥1 arg
(qa-err (funcall) 'wrong-number-of-arguments)
(qa-err (apply 'car '(a b)) 'wrong-number-of-arguments)

;; == apply-partially ==
(qa (funcall (apply-partially '+ 1) 2 3) 6)
(qa (funcall (apply-partially 'list 'a) 'b 'c) '(a b c))
(qa (apply (apply-partially 'list 'x) '(y z)) '(x y z))
(qa (funcall (apply-partially 'car) '(1)) 1)
(qa (functionp (apply-partially '+ 1)) t)
;; captured args come FIRST: (apply-partially 'car 1) then funcall
;; with '(2) => (car 1 2) => arity error, not a type error
(qa (condition-case e (funcall (apply-partially 'car 1) '(2))
      (error (car e)))
    'wrong-number-of-arguments)
;; no eager validation of the wrapped function
(qa (functionp (apply-partially 'qa-nosuchsym 1)) t)
(qa (condition-case e (funcall (apply-partially 'qa-nosuchsym 1) 2)
      (error (car e)))
    'void-function)
;; docstring of the partial is generated
(qa (stringp (documentation (apply-partially '+ 1) t)) t)
(qa-err (apply-partially) 'wrong-number-of-arguments)
;; the partial has arity (0 . many)
(qa (funcall (apply-partially 'list)) nil)

;; == funcall-with-delayed-message — (TIMEOUT MESSAGE FUNCTION) ==
(qa (funcall-with-delayed-message 0.001 "hello" (lambda () 'done))
       'done)
;; exactly 3 args: extra trailing args are an arity error
(qa-err (funcall-with-delayed-message 2 "m" '+ 1 2)
        'wrong-number-of-arguments)
(qa-err (funcall-with-delayed-message 2 "m") 'wrong-number-of-arguments)
(qa-err (funcall-with-delayed-message) 'wrong-number-of-arguments)
;; FUNCTION may be a lambda or 0-arg closure
(qa (funcall-with-delayed-message 0 "m" (lambda () (+ 2 3))) 5)

;; == closures ==
;; lexical eval builds a real closure; it captures the env
(qa (let ((f (eval '(let ((n 5)) (lambda (x) (+ x n))) t)))
      (funcall f 10))
    15)
;; closure is `interpreted-function' typed, NOT a cons
(qa (let ((c (eval '(let ((n 7)) (lambda (x) (+ x n))) t)))
      (list (type-of c) (consp c) (interpreted-function-p c)))
    '(interpreted-function nil t))
;; aref 0 = arglist, aref 1 = body (a LIST of forms)
(qa (let ((c (eval '(let ((n 7)) (lambda (x) (+ x n))) t)))
      (list (aref c 0) (aref c 1)))
    '((x) ((+ x n))))
;; nested lambdas capture outer args
(qa (eval '(funcall (funcall (lambda (x) (lambda (y) (+ x y))) 3) 4) t)
    7)
;; lambda data literal is functionp but NOT an interpreted-function
;; object until evaluated
(qa (interpreted-function-p '(lambda (x) x)) nil)
(qa (functionp '(lambda (x) x)) t)
;; literal (closure …) conses are not functions in 31.x
(qa (functionp '(closure (t) (x) x)) nil)
(qa (condition-case e (funcall '(closure (t) (x) x) 1)
      (error (car e)))
    'invalid-function)

;; == functionp / subrp / macrop ==
(qa (functionp 'car) t)
(qa (functionp #'car) t)
(qa (functionp (symbol-function 'car)) t)
(qa (functionp 'qa-nosuchsym) nil)
(qa (functionp (lambda (x) x)) t)
(qa (functionp "str") nil)
(qa (functionp 5) nil)
(qa (subrp (symbol-function 'car)) t)
(qa (subrp 'car) nil)
(qa (subrp (apply-partially '+ 1)) nil)
(qa (byte-code-function-p (symbol-function 'car)) nil)
;; apply-partially returns a byte-code object like GNU's eval.c
(qa (byte-code-function-p (apply-partially '+ 1)) t)
;; macrop resolves through symbol function cells
(qa (macrop 'when) t)
(qa (macrop (symbol-function 'when)) t)
(qa (macrop 'car) nil)
;; `if' is a special form — not a macro
(qa (macrop 'if) nil)
(qa (macrop (symbol-function 'if)) nil)
;; indirect-function returns the resolved object (subr itself)
(qa (eq (indirect-function 'car) (symbol-function 'car)) t)
(qa (indirect-function 'qa-nosuchsym) nil)
;; apply/funcall reach a defun through its symbol
(qa (progn
      (defun qa-ap2-add (a b) (+ a b))
      (list (apply 'qa-ap2-add '(3 4)) (funcall 'qa-ap2-add 1 2)))
    '(7 3))
;; lambda eval'd under dynamic scope (nil) still funcalls — env unused
(qa (funcall (eval '(lambda (x) (1+ x)) nil) 4) 5)

(princ (qa--done))
