;; Special forms + macro/symbol plumbing — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - special-form-p is t for these: if let let* setq quote function
;;     catch unwind-protect condition-case while and or progn prog1
;;     defvar defconst save-current-buffer save-excursion save-restriction
;;     interactive.
;;   - defun/defmacro/throw/prog2/track-mouse/eval-when-compile are NOT
;;     specials (defun/defmacro are defined in subr.el; throw is a fn).
;;   - macroexpand does TOP-LEVEL expansion only — nested macros stay.
;;   - subrp takes the FUNCTION VALUE, not a symbol.
;;   - indirect-function returns the final function.
;;   - function-put/function-get read/write function props.

;; == surface ==
(qa (fboundp 'special-form-p) t)
(qa (fboundp 'macroexpand) t)
(qa (fboundp 'macroexpand-1) t)
(qa (fboundp 'macroexpand-all) t)
(qa (fboundp 'purecopy) t)
(qa (fboundp 'function-get) t)
(qa (fboundp 'function-put) t)
(qa (fboundp 'defalias) t)
(qa (fboundp 'fset) t)
(qa (fboundp 'indirect-function) t)
(qa (fboundp 'commandp) t)
(qa (fboundp 'byte-code-function-p) t)
(qa (fboundp 'interpreted-function-p) t)
(qa (fboundp 'symbol-file) t)
;; NOT fns
(qa (fboundp 'macroexpand-all-environment) nil)
(qa (fboundp 'macroexpand-1-environment) nil)
(qa (fboundp 'purecopy-cons) nil)
(qa (fboundp 'purecopy-string) nil)
(qa (fboundp 'symbol-function-or-function) nil)
(qa (fboundp 'interactive-only) nil)
(qa (fboundp 'macro-function) nil)

;; == special-form-p — TRUE specials ==
(qa (special-form-p 'if) t)
(qa (special-form-p 'let) t)
(qa (special-form-p 'let*) t)
(qa (special-form-p 'setq) t)
(qa (special-form-p 'quote) t)
(qa (special-form-p 'function) t)
(qa (special-form-p 'catch) t)
(qa (special-form-p 'unwind-protect) t)
(qa (special-form-p 'condition-case) t)
(qa (special-form-p 'while) t)
(qa (special-form-p 'and) t)
(qa (special-form-p 'or) t)
(qa (special-form-p 'progn) t)
(qa (special-form-p 'prog1) t)
;; GNU has these as Lisp-level macros, not specials.
(qa (special-form-p 'defun) nil)
(qa (special-form-p 'defmacro) nil)
(qa (special-form-p 'prog2) nil)
(qa (special-form-p 'track-mouse) nil)
(qa (special-form-p 'eval-when-compile) nil)
(qa (special-form-p 'when) nil)
(qa (special-form-p 'car) nil)
(qa (special-form-p 'list) nil)
(qa (special-form-p :x) nil)
(qa (special-form-p 42) nil)
(qa (special-form-p "str") nil)

;; == macroexpand-1 single-step ==
(qa (macroexpand-1 '(when x 1)) '(if x (progn 1)))
;; macroexpand only expands the TOP level — nested stays unexpanded
(qa (macroexpand '(progn (when x 1) 2))
    '(progn (when x 1) 2))
;; macroexpand returns the form unchanged if not a macro call
(qa (macroexpand '(car x)) '(car x))
(qa (macroexpand 42) 42)
(qa (macroexpand 'sym) 'sym)

;; == macroexpand-all expands nested ==
(let ((r (macroexpand-all '(progn (when x 1) 2))))
  (qa (listp r) t)
  ;; when is expanded inside
  (qa (eq (car r) 'progn) t))

;; == indirect-function follows defalias chains ==
(fset 'my-alias #'car)
;; returns the final FUNCTION (a subr), not the symbol
(qa (indirect-function 'my-alias) (symbol-function 'car))
;; chain
(defalias 'alias2 'my-alias)
(qa (indirect-function 'alias2) (symbol-function 'car))
;; on a fn — same value
(qa (indirect-function #'car) (symbol-function 'car))

;; == function-put / function-get ==
(defun myfunc () "my doc" 42)
(function-put 'myfunc 'x 99)
(qa (function-get 'myfunc 'x) 99)
;; unset prop → nil
(qa (function-get 'myfunc 'unset-prop) nil)

;; == symbol-file ==
(let ((sf (symbol-file 'car)))
  (qa (or (null sf) (stringp sf)) t))

;; == commandp ==
(defun mycmd () "c" (interactive) 1)
(qa (commandp 'mycmd) t)
(qa (commandp 'myfunc) nil)
(qa (commandp 'car) nil)
(qa (commandp 'setq) nil)
(let ((f (lambda () (interactive) 1)))
  (qa (commandp f) t))

;; == purecopy ==
(let ((orig '(a (b c))))
  (let ((c (purecopy orig)))
    (qa (equal c orig) t)))
(qa (purecopy 42) 42)
(qa (purecopy "abc") "abc")

;; == makunbound / fmakunbound ==
(defvar testvar 5)
(makunbound 'testvar)
(qa (boundp 'testvar) nil)
(fset 'testfn #'car)
(fmakunbound 'testfn)
(qa (fboundp 'testfn) nil)

;; == interpreted-function-p / byte-code-function-p ==
(let ((f (lambda (x) (+ x 1))))
  (qa (interpreted-function-p f) t)
  (qa (byte-code-function-p f) nil))
;; subr is neither
(qa (interpreted-function-p #'car) nil)
(qa (byte-code-function-p #'car) nil)

;; == subrp — takes the FUNCTION value ==
(qa (subrp (symbol-function 'car)) t)
;; on the fn object directly
(qa (subrp #'car) nil)   ; #'car evals to the fn value, not the symbol
;; not a subr
(qa (subrp (lambda (x) x)) nil)
(qa (subrp 'car) nil)    ; symbol → not a subr value

;; == functionp ==
(qa (functionp #'car) t)
(qa (functionp (lambda (x) x)) t)
(qa (functionp 'car) t)  ; symbol's fn cell is a fn
(qa (functionp 42) nil)
(qa (functionp "str") nil)

;; == setplist ==
(let ((s (make-symbol "sym")))
  (setplist s '(a 1 b 2))
  (qa (symbol-plist s) '(a 1 b 2)))

;; == set ==
(let ((s (make-symbol "dyn")))
  (set s 42)
  (qa (symbol-value s) 42))

;; == symbol-name ==
(qa (symbol-name 'abc) "abc")
(qa (symbol-name (intern "xyz-thing")) "xyz-thing")

;; == defalias chain ==
(defalias 'a1 'car)
(defalias 'a2 'a1)
(defalias 'a3 'a2)
(qa (indirect-function 'a3) (symbol-function 'car))

;; == fmakunbound chain ==
(fset 'xfn #'car)
(qa (fboundp 'xfn) t)
(fmakunbound 'xfn)
(qa (fboundp 'xfn) nil)

(princ (qa--done))
