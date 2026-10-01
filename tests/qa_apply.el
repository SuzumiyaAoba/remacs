;;; -*- lexical-binding: t -*-
;; Function application, arity, closures — GNU 31.1-verified semantics:
;;  `wrong-number-of-arguments' data is (FN COUNT): a subr invoked via its
;;  symbol reports the symbol, via apply/object reports the #<subr> object;
;;  a compiled macro-expander may report an arity cons such as (1 . 1).
;;  `funcall'/`apply' on nil/t/keywords signal void-function; on strings,
;;  numbers, vectors, old-style `closure' lists and `macro' objects signal
;;  invalid-function (data = the object; macro objects round-trip through
;;  `macro . LAMBDA').  Bare `autoload' conses signal wrong-type-argument
;;  symbolp.  `partial-apply' does not exist in 31.1; `apply-partially'
;;  prepends captured args, does not validate its function up front, yields
;;  arity (0 . many).  `funcall-interactively' never evaluates the
;;  interactive form — it applies the args it is given and only makes
;;  `called-interactively-p' true.  defmacro arglists support &rest and
;;  plain &optional but NOT default values/&key/&whole/&aux.  `macroexpand'
;;  expands outermost first; `macroexpand-all' descends into special forms.
;;  `add-hook'/`remove-hook' return the new list and bind unbound vars;
;;  hooks run newest-first; equal functions dedupe; DEPTH sorts ascending
;;  execution is larger-depth-first and appends a `t' marker.
(require 'cl-lib)

;; == helpers ==
(defun qaap-id (x) x)
(defun qaap-cmd () (interactive) (called-interactively-p 'any))
(defun qaap-2 (a b) (list a b))
(defmacro qaap-m2 (a b) `(+ ,a ,b))
(defmacro qaap-wrap (x) `(list ,x))
(defmacro qaap-outer (x) `(qaap-wrap ,x))
(defmacro qaap-restm (&rest args) `(quote ,args))
(defmacro qaap-dopt (a &optional b) `(list ,a ,b))
;; GNU's own macro-expander lambda cannot handle &optional defaults, &key,
;; &whole or &aux — keep calls to these quoted (eager expansion of a call
;; site would abort the load under GNU too).
(defmacro qaap-doptd (a &optional (b 7)) `(list ,a ,b))
(defmacro qaap-dkey (a &key (b 2)) `(list ,a ,b))
(defmacro qaap-dwhole (&whole f a) `(list (quote ,f) ,a))
(defmacro qaap-daux (a &aux (b 3)) `(list ,a ,b))

(defvar qaap-h1 nil)
(defvar qaap-h2 nil)
(defvar qaap-h3 nil)
(defvar qaap-h4 nil)
(defvar qaap-h5 nil)
(defvar qaap-h6 nil)
(defvar qaap-h7 nil)
(defvar qaap-h8 nil)
(defvar qaap-h9 nil)
(defvar qaap-h10 nil)
(defvar qaap-h11 nil)
(defvar qaap-h12 nil)
(defvar qaap-h13 nil)
(defvar qaap-h14 nil)
(defvar qaap-h15 nil)
(defvar qaap-h16 nil)
(defvar qaap-h17 nil)
(defvar qaap-h18 nil)
(defvar qaap-h19 nil)
(defvar qaap-h20 nil)

;; == funcall: wrong-number-of-arguments payloads ==
;; Symbol-called subr: data is (SYMBOL COUNT).
;; errdata is (FUNCTION-SYMBOL COUNT); resolved to subr when via #'
(qa-err-data (funcall #'cons 1) 'wrong-number-of-arguments
             (list (symbol-function 'cons) 1))
(qa-err-data (funcall #'cons 1 2 3) 'wrong-number-of-arguments
             (list (symbol-function 'cons) 3))
(qa-err-data (funcall #'eq) 'wrong-number-of-arguments
             (list (symbol-function 'eq) 0))
;; A subr OBJECT reports the #<subr> itself — compare via subr-name.
(qa (condition-case e (funcall (symbol-function 'car))
      (error (list (car e) (subr-name (nth 1 e)) (nth 2 e))))
    '(wrong-number-of-arguments "car" 0))
;; Lambdas/closures report the compiled closure object + count.
(qa (condition-case e (funcall (lambda (a b) a) 1)
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))
(qa (condition-case e (funcall (lambda (a &optional b) a) 1 2 3)
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 3))
(qa (condition-case e (funcall (lambda (a &optional b) a))
      (error (nth 2 e)))
    0)
;; &rest lambdas take any count.
(qa (funcall (lambda (a &rest r) r) 1 2 3 4) '(2 3 4))
(qa (funcall (lambda (&rest r) r)) nil)
(qa (funcall (lambda (&optional b) (list b))) '(nil))
(qa (funcall (lambda (a &optional b) (list a b)) 7) '(7 nil))
;; Interpreted lambda forms funcall directly.
(qa (funcall '(lambda (x) x) 5) 5)

;; == funcall: invalid-function / void-function payloads ==
;; A special-form symbol reports the #<subr> object, not the symbol.
(qa (condition-case e (funcall 'if t 1)
      (error (list (car e) (subr-name (nth 1 e)))))
    '(invalid-function "if"))
;; A macro OBJECT (macro . LAMBDA) is reported as (macro lambda ...).
(qa-err-data (funcall '(macro . (lambda (a) a)) 1)
             'invalid-function '((macro lambda (a) a)))
(qa (condition-case e (funcall (symbol-function 'when) t 1)
      (error (list (car e) (equal (nth 1 e) (symbol-function 'when)))))
    '(invalid-function t))
;; Old-style closure lists are not functions in 31.1.
(qa-err-data (funcall '(closure (t) (x) x) 5)
             'invalid-function '((closure (t) (x) x)))
(qa-err-data (funcall '(closure ((y . 7) t) (x) (+ x y)) 5)
             'invalid-function '((closure ((y . 7) t) (x) (+ x y))))
;; A bare autoload cons signals wrong-type-argument symbolp.
(qa-err-data (funcall '(autoload "x" nil nil nil))
             'wrong-type-argument '(symbolp (autoload "x" nil nil nil)))
;; nil / t / keywords report void-function, other atoms invalid-function.
(qa-err-data (funcall nil 1) 'void-function '(nil))
(qa-err-data (funcall t nil) 'void-function '(t))
(qa-err-data (funcall :k 1) 'void-function '(:k))
(qa-err-data (funcall 3.5) 'invalid-function '(3.5))
(qa-err-data (funcall '(1 . 2) 3) 'invalid-function '((1 . 2)))
(qa-err-data (funcall (make-sparse-keymap)) 'invalid-function '((keymap)))
(qa (funcall #'funcall #'list 1 2) '(1 2))
(qa (funcall #'apply #'list '(1 2)) '(1 2))

;; == apply: spreading, payload shapes ==
(qa (apply '(lambda (x) (* x x)) '(5)) 25)
(qa (apply (lambda (&rest r) r) 1 '(2 3)) '(1 2 3))
(qa (let ((x 10)) (apply (lambda (a) (+ a x)) '(5))) 15)
(qa (apply #'list 1 2 nil) '(1 2))
;; apply with no trailing list checks the FUNCTION arg with listp.
(qa-err-data (apply #'car) 'wrong-type-argument '(listp car))
;; Trailing non-list object reports listp of the offending tail.
(qa-err-data (apply #'list 1 5) 'wrong-type-argument '(listp 5))
(qa-err-data (apply #'list '(1 . 2)) 'wrong-type-argument '(listp 2))
;; apply through apply: inner apply gets the spread args.
(qa-err-data (apply #'apply #'list '(1 2)) 'wrong-type-argument '(listp 2))
;; apply reports the #<subr> object for arity errors.
(qa (condition-case e (apply #'car nil)
      (error (list (car e) (subr-name (nth 1 e)) (nth 2 e))))
    '(wrong-number-of-arguments "car" 0))
(qa (condition-case e (apply #'cons '(1 2 3))
      (error (list (car e) (subr-name (nth 1 e)) (nth 2 e))))
    '(wrong-number-of-arguments "cons" 3))
(qa (condition-case e (apply (symbol-function 'cons) '(1))
      (error (list (car e) (subr-name (nth 1 e)) (nth 2 e))))
    '(wrong-number-of-arguments "cons" 1))
(qa-err-data (apply 'when '(t 1)) 'invalid-function '(when))
(qa-err-data (apply '(macro . (lambda (a) a)) '(1))
             'invalid-function '((macro lambda (a) a)))
(qa-err-data (apply '(closure (t) (x) x) '(5))
             'invalid-function '((closure (t) (x) x)))
(qa-err-data (apply t nil) 'void-function '(t))
;; apply on a lambda with too few args reports the closure object.
(qa (condition-case e (apply (lambda (a b) a) '(1))
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))

;; == apply-partially / partial-apply ==
(qa (fboundp 'partial-apply) nil)
;; Captured args come FIRST; later args append.
(qa (funcall (apply-partially #'list 'x 'y) 'z) '(x y z))
(qa (funcall (apply-partially #'- 10 2) 3) 5)
(qa (funcall (apply-partially #'concat "a" "b") "c") "abc")
(qa (funcall (apply-partially #'list)) nil)
(qa (apply (apply-partially #'+ 1) 2 '(3 4)) 10)
(qa (funcall (apply-partially (apply-partially #'list 1) 2) 3) '(1 2 3))
;; The function is not validated at construction time.
(qa (functionp (apply-partially 5 1)) t)
(qa-err-data (funcall (apply-partially 5 1) 2) 'invalid-function '(5))
(qa-err-data (funcall (apply-partially 'qaap-undef 1) 2) 'void-function '(qaap-undef))
(qa-err-data (funcall (apply-partially 'when t) 1) 'invalid-function '(when))
;; Result is a compiled &rest function.
(qa (func-arity (apply-partially #'cons 'a)) '(0 . many))
(qa-err (subr-arity (apply-partially #'cons 'a)) 'wrong-type-argument)
(qa (interactive-form (apply-partially #'ignore)) nil)
(qa (documentation (apply-partially #'cons 'a)) "\n\n(fn &rest ARGS2)")
(qa (eq (apply-partially #'cons 'a) (apply-partially #'cons 'a)) nil)
;; Excess args still hit the underlying function's arity check.
(qa (condition-case e (funcall (apply-partially #'car 1) 2)
      (error (list (car e) (subr-name (nth 1 e)) (nth 2 e))))
    '(wrong-number-of-arguments "car" 2))

;; == func-arity ==
(qa (func-arity '(lambda () 1)) '(0 . 0))
(qa (func-arity '(lambda (&rest r) r)) '(0 . many))
(qa (func-arity '(lambda (&optional a) a)) '(0 . 1))
(qa (func-arity '(lambda (a &optional b c) a)) '(1 . 3))
(qa (func-arity (let ((x 1)) (lambda (a &optional b) (list a b x)))) '(1 . 2))
(qa (func-arity '(macro . (lambda (a b) a))) '(2 . 2))
;; Special forms report unevalled.
(qa (func-arity 'if) '(2 . unevalled))
(qa (func-arity 'setq) '(0 . unevalled))
;; Old-style closure lists are rejected.
(qa-err-data (func-arity '(closure (t) (a) a))
             'invalid-function '((closure (t) (a) a)))
(qa-err-data (func-arity '(autoload "x" nil nil nil))
             'wrong-type-argument '(symbolp (autoload "x" nil nil nil)))
;; Aliases resolve to the target's arity.
(progn
  (defalias 'qaap-aly 'cons)
  (qa (func-arity 'qaap-aly) '(2 . 2)))
;; An autoloaded function's arity forces the load.
(progn
  (autoload 'qaap-auto "no-such-file-qaap" nil nil nil)
  (qa (fboundp 'qaap-auto) t)
  (qa (indirect-function 'qaap-auto) '(autoload "no-such-file-qaap" nil nil nil))
  (qa-err (func-arity 'qaap-auto) 'file-missing)
  (qa-err (funcall 'qaap-auto) 'file-missing))

;; == subr-arity ==
(qa (subr-arity (symbol-function 'eq)) '(2 . 2))
;; A macro's function cell is not a subr.
(qa-err (subr-arity (symbol-function 'when)) 'wrong-type-argument)

;; == functionp / macrop / special-form-p on odd objects ==
(qa (functionp '(closure (t) (x) x)) nil)
(qa (functionp '(autoload "f" nil nil t)) nil)
(qa (functionp (make-sparse-keymap)) nil)
(qa (functionp (make-finalizer #'ignore)) nil)
(qa (functionp nil) nil)
(qa (functionp (let ((x 1)) (lambda () x))) t)
(qa (functionp (apply-partially #'cons 'a)) t)
(qa (macrop '(macro . (lambda (a) a))) t)
(qa (macrop (symbol-function 'when)) t)
(qa (macrop 5) nil)
(qa (special-form-p (symbol-function 'if)) t)

;; == function-get / function-put ==
(qa (function-put 'qaap-gs 'prop 'val) 'val)
(qa (function-get 'qaap-gs 'prop) 'val)
(qa (function-get 'qaap-nosuch-sym 'prop) nil)
(qa (function-get 5 'prop) nil)
(qa-err-data (function-put 5 'p 'v)
             'wrong-type-argument '((symbolp symbol-with-pos-p) 5))
(qa (function-get 'car 'side-effect-free) nil)
(qa (function-get 'car 'doc-string-elt) nil)

;; == defalias / fset / symbol-function / fmakunbound / indirect-function ==
;; Aliases store the target symbol unresolved.
(progn
  (defalias 'qaap-a1 'qaap-a2)
  (qa (symbol-function 'qaap-a1) 'qaap-a2)
  (qa (fboundp 'qaap-a1) t)
  (qa (indirect-function 'qaap-a1) nil)
  (qa (indirect-function 'qaap-a1 t) nil))
(progn
  (defalias 'qaap-b1 'qaap-b2)
  (defalias 'qaap-b2 'car)
  (qa (symbol-function 'qaap-b1) 'qaap-b2)
  (qa-same (indirect-function 'qaap-b1) (symbol-function 'car))
  (qa (subrp (indirect-function 'qaap-b1)) t))
;; Non-symbol atoms pass through indirect-function unchanged.
(qa (indirect-function [1 2]) [1 2])
(qa (indirect-function '(lambda (z) z)) '(lambda (z) z))
(qa (indirect-function 5) 5)
;; defalias to nil: stores nil, so fboundp drops and calls go void.
(progn
  (qa (defalias 'qaap-n nil) 'qaap-n)
  (qa (fboundp 'qaap-n) nil)
  (qa-err-data (funcall 'qaap-n) 'void-function '(qaap-n)))
;; fset accepts arbitrary objects; calls then fail at funcall time.
(progn
  (qa (fset 'qaap-nf 5) 5)
  (qa (symbol-function 'qaap-nf) 5)
  (qa-err-data (funcall 'qaap-nf 1) 'invalid-function '(qaap-nf)))
(progn
  (fset 'qaap-nl '(lambda (x) (* x x)))
  (qa (funcall 'qaap-nl 4) 16))
;; fmakunbound returns the symbol; works on undefined and subr-valued cells.
(qa (fmakunbound 'qaap-undef) 'qaap-undef)
(progn
  (fset 'qaap-car2 (symbol-function 'car))
  (qa (qaap-car2 '(7)) 7)
  (qa (fmakunbound 'qaap-car2) 'qaap-car2)
  (qa (fboundp 'qaap-car2) nil))
(qa-err-data (symbol-function 5) 'wrong-type-argument '(symbolp 5))
(qa-err (fboundp (lambda (x) x)) 'wrong-type-argument)
(qa (defalias 'qaap-d1 'car "doc string") 'qaap-d1)

;; == macroexpand / macroexpand-1 / macroexpand-all ==
(qa (macroexpand-1 '(when t 5)) '(if t (progn 5)))
;; Calling `when' with no args fails inside its compiled expander; the
;; reported "function" is the arity cons (1 . 1).
(qa-err-data (macroexpand-1 '(when))
             'wrong-number-of-arguments '((1 . 1) 0))
(qa (macroexpand-1 '(when-let* ((x 1)) x)) '(if-let* ((x 1)) x))
;; macroexpand peels the outermost macro, then repeats on the RESULT head.
(qa (macroexpand '(qaap-wrap (qaap-wrap 1))) '(list (qaap-wrap 1)))
(qa (macroexpand '(qaap-outer 1)) '(list 1))
(qa (macroexpand-1 '(qaap-wrap (qaap-wrap 1))) '(list (qaap-wrap 1)))
(qa (macroexpand '(car x)) '(car x))
(qa (macroexpand 'sym) 'sym)
(qa (macroexpand nil) nil)
(qa (macroexpand-1 nil) nil)
(qa (macroexpand '((lambda (x) x) 1)) '((lambda (x) x) 1))
;; ENVIRONMENT alist overrides global macro definitions.
(qa (macroexpand '(qaap-wrap z)
                 '((qaap-wrap . (lambda (x) (list 'wrapped x)))))
    '(wrapped z))
;; &rest and plain &optional work in defmacro arglists.
(qa (macroexpand '(qaap-restm 1 2 3)) '(quote (1 2 3)))
(qa (macroexpand '(qaap-dopt 1)) '(list 1 nil))
(qa (macroexpand '(qaap-dopt 1 2)) '(list 1 2))
(qa (qaap-dopt 9) '(9 nil))
;; ...but defaults/&key/&whole/&aux are not handled by the expander.
(qa-err (macroexpand '(qaap-doptd 1)) 'invalid-function)
(qa-err (macroexpand '(qaap-dkey 1 :b 9)) 'invalid-function)
(qa (condition-case e (macroexpand '(qaap-dwhole 1))
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))
(qa (condition-case e (macroexpand '(qaap-daux 1))
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))
;; Arity errors in macro calls report the compiled expander + count.
(qa (condition-case e (macroexpand '(qaap-m2 1 2 3))
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 3))
;; macroexpand-all descends into special forms and closures.
(qa (macroexpand-all '(when t (when nil 1))) '(if t (progn (if nil (progn 1)))))
(qa (macroexpand-all '(let ((a 1)) a)) '(let ((a 1)) a))
(qa (macroexpand-all '(quote (when x))) '(quote (when x)))
(qa (macroexpand-all '(car x)) '(car x))
(qa (macroexpand-all 'sym) 'sym)
(qa (macroexpand-all '(setq qaap-x (when t 1))) '(setq qaap-x (if t (progn 1))))
(qa (macroexpand-all '(lambda (x) (when x 1)))
    '(function (lambda (x) (if x (progn 1)))))
;; funcall of a literal lambda is beta-reduced to a let.
(qa (macroexpand-all '(funcall #'(lambda (x) (when x x)) 5))
    '(let ((x 5)) (if x (progn x))))
(qa (macroexpand-all '(defun qaap-f (x) (when x 1)))
    '(defalias 'qaap-f #'(lambda (x) (if x (progn 1)))))
;; cond is a special form in 31.1 — untouched.
(qa (macroexpand-1 '(cond (a 1) (t 2))) '(cond (a 1) (t 2)))

;; == eval of exotic function positions ==
(qa-err-data (eval '(closure (t) () 1) t) 'void-function '(closure))
(qa (eval '((lambda (x) (+ x 1)) 41) t) 42)

;; == lexical closures ==
;; Closures share their captured cell: setq is visible afterwards.
(qa (let ((x 1))
      (let ((f (lambda () x)))
        (setq x 9)
        (funcall f)))
    9)
;; Two closures over one binding see the same updates.
(defun qaap-two-closures ()
  (let ((x 1) f g)
    (setq f (lambda () x))
    (setq x 5)
    (setq g (lambda () x))
    (list (funcall f) (funcall g))))
(qa (qaap-two-closures) '(5 5))
;; Each factory call makes a fresh cell.
(defun qaap-factory ()
  (let ((mk (lambda () (let ((n 0)) (lambda () (setq n (1+ n)))))))
    (let ((c1 (funcall mk)) (c2 (funcall mk)))
      (funcall c1) (funcall c1)
      (list (funcall c1) (funcall c2)))))
(qa (qaap-factory) '(3 1))
;; dolist creates a fresh binding per iteration.
(defun qaap-dolist-cells ()
  (let (fs)
    (dolist (i '(1 2 3))
      (push (lambda () i) fs))
    (mapcar #'funcall (nreverse fs))))
(qa (qaap-dolist-cells) '(1 2 3))
;; An explicit let inside the loop body is also per-iteration.
(defun qaap-dolist-inner-let ()
  (let (fs)
    (dolist (i '(1 2 3))
      (let ((j i)) (push (lambda () j) fs)))
    (mapcar #'funcall (nreverse fs))))
(qa (qaap-dolist-inner-let) '(1 2 3))
;; ...but setq mutates ONE shared cell.
(defun qaap-setq-shared ()
  (let ((x 0) fs)
    (dolist (i '(1 2 3))
      (setq x i)
      (push (lambda () x) fs))
    (mapcar #'funcall (nreverse fs))))
(qa (qaap-setq-shared) '(3 3 3))
;; Function arguments are captured cells too.
(defun qaap-arg-cell ()
  (let ((mk (lambda (x) (lambda () x))))
    (list (funcall (funcall mk 'a)) (funcall (funcall mk 'b)))))
(qa (qaap-arg-cell) '(a b))
;; defalias inside a let still captures the lexical binding.
(progn
  (let ((v 'cap))
    (defalias 'qaap-captured (lambda () v))
    (qa (funcall 'qaap-captured) 'cap)))
;; Closure identity.
(qa (let ((x 1)) (eq (lambda () x) (lambda () x))) nil)
(qa (let* ((x 1) (f (lambda () x))) (eq f f)) t)
;; two closures, one mutating the shared cell
(qa (let ((x 0) get)
      (let ((inc (lambda () (setq x (1+ x)))))
        (setq get (lambda () x))
        (funcall inc)
        (funcall inc)
        (funcall get)))
    2)

;; == funcall-interactively ==
;; Args are applied literally; the interactive form is NOT evaluated.
(qa (funcall-interactively (lambda (x) (interactive "P") x) 9) 9)
(qa (condition-case e (funcall-interactively (lambda (x) (interactive "P") x))
        (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 0))
;; ...but called-interactively-p still sees an interactive call.
(qa (funcall-interactively (lambda () (called-interactively-p 'any))) t)
;; Non-interactive calls behave like apply: trailing args need listp shape.
(qa-err-data (funcall-interactively 'car 1) 'wrong-type-argument '(listp 1))
(qa-err-data (funcall-interactively 'qaap-undef) 'void-function '(qaap-undef))
(qa-err-data (funcall-interactively 5) 'invalid-function '(5))
(qa (condition-case e (funcall-interactively (lambda (a b) (list a b)) 1)
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))

;; == commandp / interactive-form / call-interactively / c-i-p ==
;; FOR-CALL-INTERACTIVELY=t rejects keyboard-macro key sequences.
(qa (commandp "abc" t) nil)
(qa (commandp [?a] t) nil)
(qa (commandp '(lambda () (interactive)) t) t)
(qa (commandp 'car t) nil)
;; Keymaps are no longer commands.
(qa (commandp (make-sparse-keymap)) nil)
(qa (commandp '(macro . (lambda () (interactive)))) nil)
(qa (commandp 'qaap-undef) nil)
;; commandp resolves aliases and accepts function objects.
(progn
  (defalias 'qaap-calias 'qaap-cmd)
  (qa (commandp 'qaap-calias) t)
  (qa (commandp (symbol-function 'qaap-cmd)) t))
(qa (interactive-form nil) nil)
(qa (interactive-form 5) nil)
(qa (interactive-form (make-sparse-keymap)) nil)
;; A non-list spec is still stored verbatim.
(qa (interactive-form '(lambda () (interactive 5))) '(interactive 5))
(qa (call-interactively 'qaap-cmd) t)
(qa-err-data (call-interactively 'car) 'wrong-type-argument '(commandp car))
(qa-err-data (call-interactively 'qaap-undef) 'wrong-type-argument '(commandp qaap-undef))
;; The spec IS evaluated by call-interactively: a non-list spec fails.
(qa-err-data (call-interactively (lambda (x) (interactive 5) x))
             'wrong-type-argument '(listp 5))
;; called-interactively-p: kinds and arity.
(qa (called-interactively-p 'any) nil)
(qa (called-interactively-p 'bogus) nil)
(qa-err-data (called-interactively-p 'any 'extra)
             'wrong-number-of-arguments '(called-interactively-p 2))
(qa (call-interactively (lambda () (interactive) (called-interactively-p))) t)
(qa (call-interactively
         (lambda () (interactive) (called-interactively-p 'functions)))
       t)
;; Plain funcall of a command is not interactive.
(qa (funcall 'qaap-cmd) nil)

;; == hooks ==
;; Hooks run newest-first (default prepends).
(qa (let ((log nil))
      (add-hook 'qaap-h1 (lambda () (push 'a log)))
      (add-hook 'qaap-h1 (lambda () (push 'b log)))
      (run-hooks 'qaap-h1)
      log)
    '(a b))
;; append=t puts the new function last, so it runs last.
(qa (let ((log nil))
      (add-hook 'qaap-h2 (lambda () (push 'a log)))
      (add-hook 'qaap-h2 (lambda () (push 'b log)) t)
      (run-hooks 'qaap-h2)
      log)
    '(b a))
;; add-hook returns the new hook list and dedupes with `equal'.
(progn
  (setq qaap-h3 '(car))
  (qa (add-hook 'qaap-h3 #'ignore) '(ignore car))
  (qa (add-hook 'qaap-h3 #'ignore) '(ignore car)))
(progn
  (setq qaap-h4 '(car))
  (qa (add-hook 'qaap-h4 #'ignore t) '(car ignore)))
;; equal lambdas dedupe; different ones don't.
(progn
  (add-hook 'qaap-h5 '(lambda () 1))
  (add-hook 'qaap-h5 '(lambda () 1))
  (qa (length qaap-h5) 1))
(progn
  (add-hook 'qaap-h6 '(lambda () 1))
  (add-hook 'qaap-h6 '(lambda () 2))
  (qa (length qaap-h6) 2))
;; remove-hook returns the filtered list; missing fns change nothing.
(progn
  (setq qaap-h7 '(ignore car))
  (qa (remove-hook 'qaap-h7 #'ignore) '(car))
  (qa (remove-hook 'qaap-h7 #'cdr) '(car)))
;; An atomic value is converted to a one-element tail on add.
(progn
  (setq qaap-h8 'ignore)
  (add-hook 'qaap-h8 #'car)
  (qa qaap-h8 '(car ignore)))
(progn
  (setq qaap-h9 5)
  (add-hook 'qaap-h9 #'ignore)
  (qa qaap-h9 '(ignore 5)))
;; add-hook/remove-hook bind a previously unbound variable.
(progn
  (add-hook 'qaap-h10 #'ignore)
  (qa (boundp 'qaap-h10) t)
  (qa qaap-h10 '(ignore)))
(progn
  (remove-hook 'qaap-h11 #'ignore)
  (qa (boundp 'qaap-h11) t))
;; A bare function (not a list) as the value runs as a single-member hook.
(progn
  (setq qaap-h12 'ignore)
  (qa (run-hooks 'qaap-h12) nil))
(qa (let ((ran nil))
      (setq qaap-h13 (lambda () (setq ran 'yes)))
      (run-hooks 'qaap-h13)
      ran)
    'yes)
;; run-hooks tolerates nil/unbound but propagates member errors.
(qa (run-hooks 'qaap-h14) nil)
(qa (run-hooks 'qaap-never-bound-hook) nil)
(qa-err-data (progn (setq qaap-h15 '(qaap-undef-fn ignore))
                    (run-hooks 'qaap-h15))
             'void-function '(qaap-undef-fn))
(qa-err-data (progn (setq qaap-h16 '(5))
                    (run-hooks 'qaap-h16))
             'invalid-function '(5))
(qa-err-data (run-hooks 5) 'wrong-type-argument '(symbolp 5))
(qa (run-hooks) nil)
;; Multiple hook symbols run left to right.
(qa (let ((log nil))
      (add-hook 'qaap-h17 (lambda () (push 'h1 log)))
      (add-hook 'qaap-h18 (lambda () (push 'h2 log)))
      (run-hooks 'qaap-h17 'qaap-h18)
      log)
    '(h2 h1))
;; An error aborts the remaining hook members.
(qa (let ((log nil))
      (setq qaap-h14 (list (lambda () (signal 'arith-error nil))
                           (lambda () (push 'later log))))
      (list (condition-case e (run-hooks 'qaap-h14) (error e))
            log))
    '((arith-error) nil))
;; run-hook-with-args returns nil regardless of member return values.
(progn
  (setq qaap-h14 (list (lambda (a b) (+ a b))))
  (qa (run-hook-with-args 'qaap-h14 2 3) nil))
(qa (let ((v nil))
      (setq qaap-h14 (list (lambda () (setq v 'ran))))
      (run-hook-with-args 'qaap-h14)
      v)
    'ran)
(qa (condition-case e (progn (setq qaap-h14 (list (lambda (a b) (+ a b))))
                             (run-hook-with-args 'qaap-h14 2))
      (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))
;; until-success: first non-nil wins, later members skipped.
;; until-success: first non-nil wins.
(qa (let ((log nil))
      (setq qaap-h15 (list (lambda (x) (push 'f1 log) nil)
                           (lambda (x) (push 'f2 log) 'win)
                           (lambda (x) (push 'f3 log) 'never)))
      (list (run-hook-with-args-until-success 'qaap-h15 0) log))
    '(win (f2 f1)))
(progn
  (setq qaap-h15 (list (lambda (x) nil)))
  (qa (run-hook-with-args-until-success 'qaap-h15 5) nil))
(progn
  (setq qaap-h15 nil)
  (qa (run-hook-with-args-until-success 'qaap-h15 5) nil))
;; until-failure: first nil wins; all-non-nil/empty hooks return t.
;; until-failure: first nil wins.
(qa (let ((log nil))
      (setq qaap-h16 (list (lambda (x) (push 'f1 log) t)
                           (lambda (x) (push 'f2 log) nil)
                           (lambda (x) (push 'f3 log) 'never)))
      (list (run-hook-with-args-until-failure 'qaap-h16 0) log))
    '(nil (f2 f1)))
(progn
  (setq qaap-h16 (list (lambda (x) 1) (lambda (x) 2)))
  (qa (run-hook-with-args-until-failure 'qaap-h16 5) t))
(progn
  (setq qaap-h16 nil)
  (qa (run-hook-with-args-until-failure 'qaap-h16 5) t))
;; run-hook-wrapped exists; the last wrapper's value is returned.
(qa (fboundp 'run-hook-wrapped) t)
;; `(add-hook H f nil N)' passes N as LOCAL (truthy) → buffer-local
;; hook; the value carries the trailing `t' marker and equal-depth
;; entries stay newest-first.
(qa (let ((log nil))
         (add-hook 'qaap-h17 (lambda () (push 'd0 log)) nil 0)
         (add-hook 'qaap-h17 (lambda () (push 'dm log)) nil -50)
         (add-hook 'qaap-h17 (lambda () (push 'dp log)) nil 50)
         (run-hooks 'qaap-h17)
         log)
       '(h1 d0 dm dp))
(qa (let ((log nil))
         (add-hook 'qaap-h17b (lambda () (push 'd0 log)) nil 0)
         (add-hook 'qaap-h17b (lambda () (push 'dm log)) nil -50)
         (add-hook 'qaap-h17b (lambda () (push 'dp log)) nil 50)
         (mapcar (lambda (f) (if (symbolp f) f 'lam)) qaap-h17b))
       '(lam lam lam t))
;; Equal depths keep newest-first order.
(qa (let ((log nil))
         (add-hook 'qaap-h18 (lambda () (push 'a log)) nil 10)
         (add-hook 'qaap-h18 (lambda () (push 'b log)) nil 10)
         (run-hooks 'qaap-h18)
         log)
       '(h2 a b))
;; LOCAL=t makes the variable buffer-local; value ends in `t' to mean
;; "also run the global hook".
(progn
  (add-hook 'qaap-h19 #'ignore nil t)
  (qa (local-variable-p 'qaap-h19) t)
  (qa qaap-h19 '(ignore t))
  (qa (default-value 'qaap-h19) nil))
(progn
  (add-hook 'qaap-h20 #'car)
  (add-hook 'qaap-h20 #'ignore nil t)
  (qa qaap-h20 '(ignore t))
  (qa (default-value 'qaap-h20) '(car))
  ;; removing a local member keeps the local list and marker
  (add-hook 'qaap-h20 #'cdr nil t)
  (remove-hook 'qaap-h20 #'ignore t)
  (qa qaap-h20 '(cdr t)))

(princ (qa--done))
