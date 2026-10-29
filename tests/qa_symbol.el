;; Symbols, obarray, property lists, misc symbol machinery.

;; intern / intern-soft / symbol identity
(qa-same (intern "qasy-alpha") (intern "qasy-alpha"))
(qa-same (intern "qasy-beta") (intern-soft "qasy-beta"))
(qa (intern-soft "qasy-never-interned-zz") nil)
(qa (intern-soft 'car) 'car) ;; symbol arg accepted
(qa (symbolp (intern "qasy-gamma")) t)
(qa (symbol-name 'car) "car")
(qa (symbol-name (intern "qasy-δ")) "qasy-δ")
(qa-err (symbol-name "x") 'wrong-type-argument)

;; uninterned symbols
(qa (let ((s (make-symbol "qasy-u")))
      (list (symbolp s)
            (symbol-name s)
            (intern-soft "qasy-u")   ;; not interned (assume fresh name)
            (eq s s)
            (eq s (make-symbol "qasy-u"))))
    '(t "qasy-u" nil t nil))
(qa-prin1 (make-symbol "qasy-u2") "qasy-u2") ;; print-gensym is nil under -Q

;; ## is the empty-symbol name
(qa (symbol-name '##) "")

;; keyword: self-evaluating, boundp, constant
(qa :qasy-kw :qasy-kw)
(qa (symbol-value :qasy-kw) :qasy-kw)
(qa (boundp :qasy-kw) t)
(qa-err (setq :qasy-kw 1) 'setting-constant)
(qa-err (set :qasy-kw 2) 'setting-constant)
(qa-err (setq nil 1) 'setting-constant)
(qa-err (setq t 2) 'setting-constant)

;; property lists on symbols
(progn
  (put 'qasy-p 'a 1)
  (put 'qasy-p 'b 2)
  (qa (get 'qasy-p 'a) 1)
  (qa (get 'qasy-p 'b) 2)
  (qa (get 'qasy-p 'c) nil)
  (qa (symbol-plist 'qasy-p) '(a 1 b 2))
  (qa (plist-get (symbol-plist 'qasy-p) 'b) 2)
  ;; `put' with nil does NOT remove the key — GNU keeps a (a nil) pair
  (put 'qasy-p 'a nil)
  (qa (get 'qasy-p 'a) nil)
  (qa (symbol-plist 'qasy-p) '(a nil b 2))
  (qa (setplist 'qasy-p '(x 9)) '(x 9))
  (qa (symbol-plist 'qasy-p) '(x 9))
  (qa (setplist 'qasy-p nil) nil)
  (qa (symbol-plist 'qasy-p) nil))

;; plist-put / plist-member on a symbol's plist via setplist
(progn
  (setplist 'qasy-pp '(a 1))
  (qa (plist-get (symbol-plist 'qasy-pp) 'a) 1)
  (qa (plist-get (symbol-plist 'qasy-pp) 'z) nil)
  ;; 3rd arg is a comparison PREDICATE (Emacs 29+), not a default
  (qa (let ((pl '("k" 1))) (list (plist-get pl "k" #'equal)
                                 (plist-get pl "k")
                                 (plist-get pl 'k #'equal)))
      '(1 nil nil))
  (qa (plist-member (symbol-plist 'qasy-pp) 'a) '(a 1))
  (qa (plist-member (symbol-plist 'qasy-pp) 'z) nil)
  ;; plist-put is the plist counterpart of `put'; returns the new plist
  (qa (let ((pl nil)) (plist-put pl 'k 5)) '(k 5))
  (qa (let ((pl '(a 1))) (setq pl (plist-put pl 'a 2)) pl) '(a 2))
  (qa (let ((pl '(a 1))) (setq pl (plist-put pl 'b 2)) pl) '(a 1 b 2))
  ;; setplist + plist-put roundtrip on the symbol
  (setplist 'qasy-pp (plist-put (symbol-plist 'qasy-pp) 'b 3))
  (qa (symbol-plist 'qasy-pp) '(a 1 b 3))
  (setplist 'qasy-pp nil)
  (qa (symbol-plist 'qasy-pp) nil))

;; defconst / defvar semantics
(progn
  ;; defvar: only initializes when the var is void; re-eval keeps value
  (defvar qasy-dfv 1 "doc")
  (defvar qasy-dfv 99 "other doc")
  (qa qasy-dfv 1)
  (setq qasy-dfv 7)
  (defvar qasy-dfv)
  (qa qasy-dfv 7)
  ;; defconst: always resets the value
  (defconst qasy-dfc 1 "doc")
  (defconst qasy-dfc 99 "doc")
  (qa qasy-dfc 99)
  ;; defconst return value is the symbol
  (qa (progn (defconst qasy-dfc2 'v) 'ok) 'ok)
  (qa (eq (defconst qasy-dfc3 1) 'qasy-dfc3) t)
  (makunbound 'qasy-dfv)
  (qa (boundp 'qasy-dfv) nil))

;; function-put / function-get
(progn
  (function-put 'qasy-f 'prop 'val)
  (qa (function-get 'qasy-f 'prop) 'val)
  (qa (function-get 'qasy-f 'other) nil)
  ;; function-get's 3rd arg is AUTOLOAD, not a default
  (qa (function-get 'qasy-f 'other nil) nil)
  ;; function-get checks alias chain target
  (qa (function-put 'qasy-f 'prop 'newer) 'newer)
  (qa (function-get 'qasy-f 'prop) 'newer))

;; makunbound / boundp / fmakunbound / fboundp
(progn
  (defvar qasy-v 7)
  (qa (boundp 'qasy-v) t)
  (qa qasy-v 7)
  (makunbound 'qasy-v)
  (qa (boundp 'qasy-v) nil)
  (qa-err qasy-v 'void-variable)
  (defun qasy-fn () 3)
  (qa (fboundp 'qasy-fn) t)
  (fmakunbound 'qasy-fn)
  (qa (fboundp 'qasy-fn) nil)
  (qa-err (qasy-fn) 'void-function))

;; special-symbol machinery
(qa (special-form-p 'if) t)
(qa (special-form-p 'progn) t)
(qa (special-form-p 'car) nil)
(qa (special-form-p 'when) nil) ;; macro, not special form

;; indirect-function / defalias chain
(progn
  (defun qasy-real (x) (1+ x))
  (defalias 'qasy-alias1 'qasy-real)
  (defalias 'qasy-alias2 'qasy-alias1)
  (qa (qasy-alias2 10) 11)
  (qa-same (indirect-function 'qasy-alias2) (symbol-function 'qasy-real))
  (qa-same (indirect-function 'qasy-alias2 t) (symbol-function 'qasy-real)))
;; GNU detects the loop already at defalias time.
(progn
  (defalias 'qasy-c1 'qasy-c2)
  (qa-err (defalias 'qasy-c2 'qasy-c1) 'cyclic-function-indirection))
(fmakunbound 'qasy-c1)
(fmakunbound 'qasy-c2)

;; indirect-variable via defvaralias
(progn
  (defvar qasy-vt 5)
  (defvaralias 'qasy-va 'qasy-vt)
  (qa qasy-va 5)
  (setq qasy-va 9)
  (qa qasy-vt 9)
  (qa (boundp 'qasy-va) t))
(makunbound 'qasy-vt)

;; set / symbol-value / default-value vs buffer-local
(progn
  (defvar qasy-dv 1)
  (qa (symbol-value 'qasy-dv) 1)
  (qa (set 'qasy-dv 2) 2)
  (qa qasy-dv 2)
  (qa (default-value 'qasy-dv) 2)
  (qa (setq-default qasy-dv 8) 8)
  (qa (default-value 'qasy-dv) 8)
  (qa qasy-dv 8)) ;; not buffer-local yet, so global
(makunbound 'qasy-dv)

;; keyword/function predicates
(qa (keywordp :x) t)
(qa (keywordp 'x) nil)
(qa (functionp #'car) t)
(qa (functionp 'car) t)
(qa (functionp (lambda (x) x)) t)
(qa (functionp '(lambda (x) x)) t)
(qa (functionp 'qasy-nosuch-fn) nil)
(qa (functionp 5) nil)

;; subrp on builtins, nil on lambdas
(qa (subrp (symbol-function 'car)) t)
(qa (subrp (symbol-function 'when)) nil)
(qa (subrp (lambda (x) x)) nil)

;; symbol-function basics
(qa (subrp (symbol-function 'format)) t)
(qa (symbol-function 'qasy-void-fn-zz) nil)

;; obarray bits
(qa (obarrayp obarray) t)
(qa (let ((ob (make-vector 7 0)))
      (let ((s (intern "qasy-ob" ob)))
        (eq (intern-soft "qasy-ob" ob) s)))
    t)
(qa (let ((ob (make-vector 7 0))) (intern "qasy-ob2" ob) (unintern "qasy-ob2" ob) (intern-soft "qasy-ob2" ob)) nil)
(qa (mapatoms (lambda (s) (eq s 'car))) nil) ;; mapatoms returns nil
(qa (let ((n 0)) (mapatoms (lambda (_) (setq n (1+ n))) (make-vector 3 0)) n) 0)

;; gentemp/cl-gentemp are not in -Q; gensym is cl-lib
(require 'cl-lib)
(qa (string-prefix-p "G" (symbol-name (cl-gensym))) t)
(qa (let ((s (cl-gensym "qa"))) (and (symbolp s) (string-prefix-p "qa" (symbol-name s)) (null (intern-soft (symbol-name s))))) t)
(qa (string-prefix-p "qasyG" (symbol-name (cl-gentemp "qasyG"))) t)
(qa (eq (cl-gensym "z") (cl-gensym "z")) nil)

;; apropos-internal? internal; skip. bound-and-true-p macro
(qa (bound-and-true-p most-positive-fixnum) most-positive-fixnum)
(qa (bound-and-true-p qasy-no-such-var-zz) nil)
(progn
  (defvar qasy-bt2 7)
  (qa (bound-and-true-p qasy-bt2) 7)
  (setq qasy-bt2 nil)
  (qa (bound-and-true-p qasy-bt2) nil))

;; setplist/plist odd handling
(qa-err (symbol-plist 5) 'wrong-type-argument)
(qa-err (intern 5) 'wrong-type-argument)
(qa-err (unintern 5) 'wrong-number-of-arguments)

;; builtin symbol plist prop known to GNU
(qa (get 'save-excursion 'lisp-indent-function) 0)
(qa (get 'progn 'lisp-indent-function) 0)
(qa (get 'prog1 'lisp-indent-function) 1)
(qa (get 'if 'lisp-indent-function) 2)
(qa (get 'defun 'doc-string-elt) 3)
(qa (get 'lambda 'doc-string-elt) 2)

;; defvar/doc-string property via documentation-property
(progn
  (defvar qasy-docvar 1 "my doc")
  (qa (documentation-property 'qasy-docvar 'variable-documentation) "my doc"))
(makunbound 'qasy-docvar)

;; Issue #97: nil's function cell cannot hold a non-nil definition.
;; Restore the cell even on a failing pre-fix implementation.
(unwind-protect
    (progn
      (qa-err-data (fset nil 'car) 'setting-constant '(nil))
      (qa (symbol-function nil) nil)
      (qa (fboundp nil) nil))
  (fset nil nil))
(unwind-protect
    (progn
      (qa-err-data (defalias nil 'car) 'setting-constant '(nil))
      (qa (symbol-function nil) nil))
  (fset nil nil))
(unwind-protect
    (progn
      (qa-err-data (autoload nil "qa97-no-such-file") 'setting-constant '(nil))
      (qa (symbol-function nil) nil))
  (fset nil nil))
;; Autoload's defalias path records its attempted definition before fset
;; fails, but argument-validation failures do not change load history.
(unwind-protect
    (qa (let ((current-load-list (list "qa97.el")))
          (condition-case nil (autoload nil "qa97-no-such-file") (error nil))
          current-load-list)
        '((defun) "qa97.el"))
  (fset nil nil))
(qa (let ((current-load-list (list "qa97.el")))
      (condition-case nil (autoload nil 123) (error nil))
      current-load-list)
    '("qa97.el"))
(qa (fset nil nil) nil)
(qa (defalias nil nil) nil)
(qa-err-data (fmakunbound nil) 'setting-constant '(nil))
(qa-err-data (fmakunbound t) 'setting-constant '(t))
;; Autoload validates arguments before attempting to install its definition.
(qa-err-data (autoload nil 123) 'wrong-type-argument '(stringp 123))
(qa-err-data (autoload 123 123) 'wrong-type-argument '(symbolp 123))
(qa-err-data (fset 123 nil) 'wrong-type-argument '(symbolp 123))
;; `fset' must reject a function alias cycle before changing the cell.
(let ((qasy-fset-cycle-a (make-symbol "qasy-fset-cycle-a"))
      (qasy-fset-cycle-b (make-symbol "qasy-fset-cycle-b")))
  (fset qasy-fset-cycle-a qasy-fset-cycle-b)
  (qa-err-data (fset qasy-fset-cycle-b qasy-fset-cycle-a)
               'cyclic-function-indirection (list qasy-fset-cycle-b))
  (qa (symbol-function qasy-fset-cycle-b) nil)
  (qa (indirect-function qasy-fset-cycle-a) nil))
;; Existing defun validation and the shared defalias path stay protected.
(unwind-protect
    (progn
      (qa-err (defun nil () t) 'error)
      (qa (symbol-function nil) nil))
  (fset nil nil))
(unwind-protect
    (progn
      (qa-err-data (defmacro nil () t) 'setting-constant '(nil))
      (qa (symbol-function nil) nil))
  (fset nil nil))
(unwind-protect
    (qa-err-data (funcall (byte-compile (lambda (s) (fset s 'car))) nil)
                 'setting-constant '(nil))
  (fset nil nil))
;; t and keywords are not protected against fset/defalias/autoload.
(let ((old (symbol-function t)))
  (unwind-protect
      (progn
        (qa (fset t 123) 123)
        (qa (symbol-function t) 123)
        (qa (defalias t 'car) t)
        (qa (symbol-function t) 'car)
        (qa (autoload t "qa97-no-such-file") nil)
        (qa (symbol-function t) 'car)
        (fset t nil)
        (qa (autoload t "qa97-no-such-file") t)
        (qa (autoloadp (symbol-function t)) t))
    (fset t old)))
(unwind-protect
    (progn
      (qa (fset :qa97-fn 123) 123)
      (qa (defalias :qa97-fn 'car) :qa97-fn)
      (qa (fmakunbound :qa97-fn) :qa97-fn)
      (qa (autoload :qa97-fn "qa97-no-such-file") :qa97-fn))
  (fmakunbound :qa97-fn))
;; Checks use symbol identity, not the printed name.
(dolist (name '("nil" "t" ":qa97"))
  (let ((s (make-symbol name)))
    (qa (fset s 'car) 'car)
    (qa (eq (fmakunbound s) s) t)))

(princ (qa--done))
