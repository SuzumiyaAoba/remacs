;; Documentation/arity/events — GNU 31.1 pins:
;;   - `subr-arity' returns (MIN . MAX) or (MIN . many); unevalled
;;     special forms report (N . unevalled).
;;   - `func-arity' accepts symbols AND function objects; special forms
;;     report unevalled; non-functions → invalid-function.
;;   - `text-char-description' renders a char as its ^X/M-/literal form.
;;   - `event-basic-type' strips modifiers; `event-modifiers' lists them.
;;   - `single-key-description' produces "C-x" style names.
;;   - `autoloadp' on an unloaded autoload → t; the car is `autoload';
;;     after require it's a real function.
;;   - `interactive-form'/'commandp' on macros → nil.
;;   - `documentation' on nil → void-function error.
;;   - `documentation-property'/'get' retrieves stored docstrings.

;; == subr-arity ==
(qa (subr-arity (symbol-function 'car)) '(1 . 1))
(qa (subr-arity (symbol-function 'format)) '(1 . many))
(qa-err (subr-arity (lambda (x) x)) 'wrong-type-argument)
(qa-err (subr-arity 'not-a-function-z) 'wrong-type-argument)

;; == func-arity ==
(qa (func-arity (symbol-function 'car)) '(1 . 1))
(qa (func-arity 'car) '(1 . 1))
(qa (func-arity (symbol-function 'list)) '(0 . many))
;; special forms report unevalled
(qa (func-arity (symbol-function 'if)) '(2 . unevalled))
(qa (func-arity 'if) '(2 . unevalled))
(qa (func-arity 'let) '(1 . unevalled))
;; lambdas
(qa (func-arity (lambda (a b &optional c) (list a b c))) '(2 . 3))
(qa (func-arity (lambda (a &rest r) r)) '(1 . many))
(qa (func-arity (lambda () 1)) '(0 . 0))
;; non-function → invalid-function
(qa-err (func-arity 5) 'invalid-function)
(qa-err (func-arity "str") 'invalid-function)

;; == documentation ==
(qa (stringp (documentation 'car)) t)
(qa (stringp (documentation 'defun)) t)
(qa (stringp (documentation 'let)) t)
(qa-err (documentation 'nil) 'void-function)
;; function with no docstring → nil
(defun qa-nodoc-fn (x) x)
(qa (documentation 'qa-nodoc-fn) nil)
;; docstring stored on a function
(defun qa-doc-fn (x) "my doc" x)
(qa (documentation 'qa-doc-fn) "my doc")
;; defvar docstring lands on variable-documentation
(defvar qa-doc-var 1 "my var doc")
(qa (get 'qa-doc-var 'variable-documentation) "my var doc")
;; macro docstring
(defmacro qa-doc-macro () "macro doc" 1)
(qa (documentation 'qa-doc-macro) "macro doc")
;; documentation-property direct access
(qa (documentation-property 'qa-doc-var 'variable-documentation) "my var doc")

;; == interactive-form / commandp ==
(qa (interactive-form 'self-insert-command)
    '(interactive (list (prefix-numeric-value current-prefix-arg)
                        last-command-event)))
(qa (interactive-form 'car) nil)
(qa (interactive-form (lambda (x) (interactive "sName: ") x))
    '(interactive "sName: "))
(qa (interactive-form (lambda () (interactive (list 1))))
    '(interactive (list 1)))
;; a lambda without interactive → nil
(qa (interactive-form (lambda (x) x)) nil)
;; macros are never commands
(defmacro qa-dm-cmd () '(interactive) 1)
(qa (commandp 'qa-dm-cmd) nil)
(qa (interactive-form 'qa-dm-cmd) nil)
;; commandp basics
(qa (commandp 'self-insert-command) t)
(qa (commandp 'car) nil)
(qa (commandp '(lambda () (interactive) 1)) t)
(qa (commandp '(lambda (x) x)) nil)

;; == autoloadp ==
(qa (autoloadp (symbol-function 'format-spec)) t)
;; autoload object shape: (autoload FILE DOC INTERACTIVE TYPE)
(qa (car (symbol-function 'format-spec)) 'autoload)
;; require resolves the autoload into a function
(qa (progn (require 'format-spec)
           (autoloadp (symbol-function 'format-spec)))
    nil)
(qa (progn (require 'format-spec)
           (functionp 'format-spec))
    t)
;; non-autoload objects → nil
(qa (autoloadp 'car) nil)
(qa (autoloadp nil) nil)
(qa (autoloadp '(autoload "x" 1)) t)

;; == macrop / functionp ==
(qa (macrop 'qa-doc-macro) t)
(qa (functionp 'qa-doc-macro) nil)
(qa (fboundp 'qa-doc-macro) t)
(qa (macrop 'car) nil)
(qa (functionp 'car) t)
(qa (macrop '(macro . (lambda () 1))) t)
;; special-form-p: macros are NOT special forms
(qa (special-form-p 'qa-doc-macro) nil)
(qa (special-form-p 'if) t)
(qa (special-form-p 'car) nil)

;; == single-key-description ==
(qa (single-key-description ?a) "a")
(qa (single-key-description ?\C-x) "C-x")
(qa (single-key-description ?\M-y) "M-y")
(qa (single-key-description 'return) "<return>")
(qa (single-key-description 'tab) "<tab>")
(qa (single-key-description 'f1) "<f1>")
;; with shift modifier on a symbol event
(qa (single-key-description 'S-return) "S-<return>")

;; == event-basic-type / event-modifiers ==
(qa (event-basic-type ?\C-x) 120)
(qa (event-modifiers ?\C-x) '(control))
(qa (event-basic-type 'f1) 'f1)
(qa (event-basic-type 'mouse-1) 'mouse-1)
(qa (event-modifiers 'C-f1) '(control))
(qa (event-basic-type 'C-f1) 'f1)
(qa (event-modifiers 'M-S-left) '(meta shift))

;; == text-char-description ==
(qa (text-char-description ?a) "a")
(qa (text-char-description ?\C-x) "^X")
(qa (text-char-description 233) "é")
;; 1-arg only
(qa-err (text-char-description 233 'latin-1) 'wrong-number-of-arguments)

;; == function-documentation / help fns exist ==
(qa (fboundp 'describe-bindings) t)
(qa (fboundp 'apropos-internal) t)
(qa (fboundp 'describe-function) t)
(qa (fboundp 'describe-variable) t)
;; == indirection / defalias ==
(defalias 'qa-alias 'car)
(qa (eq (indirect-function 'qa-alias)
        (symbol-function 'car))
    t)
(qa (funcall 'qa-alias '(1 2)) 1)
;; alias of alias follows the chain
(defalias 'qa-alias2 'qa-alias)
(qa (eq (indirect-function 'qa-alias2)
        (symbol-function 'car))
    t)
;; defalias creating a loop signals cyclic-function-indirection AT the
;; defalias call itself (GNU detects at creation)
(defalias 'qa-loop-a 'qa-loop-b)
(qa-err (defalias 'qa-loop-b 'qa-loop-a) 'cyclic-function-indirection)
;; fmakunbound clears the cell — subrs unbind without error too.
;; Use a scratch alias: defalias, then fmakunbound the alias.
(progn
  (defun qa-tmp-fn () 1)
  (fmakunbound 'qa-tmp-fn)
  (qa (fboundp 'qa-tmp-fn) nil))
;; fmakunbound on unbound returns the SYMBOL (not nil)
(qa (fmakunbound 'never-bound-fn) 'never-bound-fn)
;; fmakunbound on a scratch alias bound to a subr works
(progn
  (defalias 'qa-scratch-subr 'car)
  (fmakunbound 'qa-scratch-subr)
  (qa (fboundp 'qa-scratch-subr) nil))
;; NOTE: fmakunbound on CORE subrs (car etc.) succeeds and breaks the
;; interpreter — GNU crash trap, not probed.

;; == setplist / symbol-plist / get / put ==
(let ((s (make-symbol "pl")))
  (put s 'k1 'v1)
  (put s 'k2 'v2)
  (qa (symbol-plist s) '(k1 v1 k2 v2))
  (setplist s '(a b))
  (qa (symbol-plist s) '(a b))
  (qa (get s 'a) 'b)
  (qa (get s 'missing) nil))
;; put on nil-valued prop
(let ((s (make-symbol "pl2")))
  (put s 'x nil)
  (qa (symbol-plist s) '(x nil)))

(princ (qa--done))
