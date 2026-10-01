;; Type predicates / defsubst / proclaim / permanent-local — GNU 31.1 pins.
;;
;; GNU truths pinned:
;;   - Built-in subrs are `subr' objects — byte-code-function-p,
;;     compiled-function-p, native-comp-function-p ALL return nil on them
;;     (they describe Lisp-compiled code, not C subrs).
;;   - (eval '(lambda ...) t) returns a CLOSURE — still not byte-code.
;;   - zerodp / proclaim / check-declare are NOT fbound at -Q batch.
;;   - defsubst creates an inline macro: macroexpand returns the CALL
;;     UNEXPANDED (defsubst defers expansion to compile time; at eval
;;     the macroexpand result is the original form? probe-verified:
;;     macroexpand '(qa-ds 5) → (qa-ds 5)).  Function cell evaluates.
;;   - permanent-local-variable-p reads the `permanent-local' symbol
;;     property; fill-column has it set.
;;   - symbol-file on a builtin → string path or nil; on a fresh
;;     defun'd symbol → nil.

;; == booleanp ==
(qa (booleanp t) t)
(qa (booleanp nil) t)
(qa (booleanp 0) nil)
(qa (booleanp 'sym) nil)
(qa (booleanp "x") nil)

;; == subr/compiled predicates on builtins ==
;; `car' is a subr — none of the Lisp-compiled predicates match
(qa (byte-code-function-p (symbol-function 'car)) nil)
;; compiled-function-p is t on C subrs too (GNU pin)
(qa (compiled-function-p (symbol-function 'car)) t)
(qa (native-comp-function-p (symbol-function 'car)) nil)
;; a lexical closure: not byte-code either (batch -Q has no .elc)
(qa (byte-code-function-p (eval '(lambda (x) x) t)) nil)
(qa (compiled-function-p (eval '(lambda (x) x) t)) nil)
;; non-function args → nil
(qa (byte-code-function-p 'sym) nil)
(qa (byte-code-function-p nil) nil)

;; == thread/mutex/cond-var types ==
(qa (threadp (current-thread)) t)
(qa (threadp 'x) nil)
(qa (threadp nil) nil)
(qa (mutexp (make-mutex)) t)
(qa (mutexp 'x) nil)
(qa (condition-variable-p (make-condition-variable (make-mutex))) t)
(qa (condition-variable-p 'x) nil)

;; == user-pointer-p: NOT fbound in this build ==
(qa (fboundp 'user-pointer-p) nil)

;; == subr-name ==
(qa (subr-name (symbol-function 'car)) "car")
(qa (subr-name (symbol-function 'goto-char)) "goto-char")

;; == func-arity shapes ==
(qa (func-arity 'car) '(1 . 1))
(qa (func-arity 'list) '(0 . many))
(qa (func-arity 'when) '(1 . many))
;; a lambda's arity
(qa (func-arity (lambda (a b &optional c) nil)) '(2 . 3))
(qa (func-arity (lambda (&rest r) nil)) '(0 . many))

;; == not-fbound fns in batch -Q ==
(qa (fboundp 'zerodp) nil)
(qa (fboundp 'proclaim) nil)
(qa (fboundp 'check-declare) nil)

;; == defsubst ==
(defsubst qa-ds (x) (* x 2))
;; function cell evaluates the definition
(qa (qa-ds 5) 10)
;; defsubst's macroexpand yields the inline-expanded body? GNU pin:
;; at -Q eval time, macroexpand returns the call UNEXPANDED — the
;; inline rule is compile-time only.
(qa (macroexpand '(qa-ds 5)) '(qa-ds 5))
;; defsubst'd fn is a function, not a macro at runtime
(qa (functionp 'qa-ds) t)

;; == proclaim (special declarations) — not fbound at -Q ==
(qa (fboundp 'proclaim) nil)

;; == permanent-local prop (permanent-local-variable-p is not fbound;
;;    use `get' on the symbol property) ==
(put 'qa-perm-var 'permanent-local t)
(qa (get 'qa-perm-var 'permanent-local) t)
;; buffer-read-only carries the property (fill-column does NOT)
(qa (get 'buffer-read-only 'permanent-local) t)
;; ordinary var → nil
(qa (get 'qa-ordinary-var 'permanent-local) nil)

;; == symbol-file ==
;; builtins have a file (loaddefs dump records it as nil or C source)
(qa (or (null (symbol-file 'car)) (stringp (symbol-file 'car))) t)
;; a just-defsubst'd symbol → nil (no file recorded yet)
(qa (symbol-file 'qa-ds) nil)
;; a defun'd symbol
(defun qa-sf-test (x) x)
(qa (symbol-file 'qa-sf-test) nil)

;; == type-of on misc objects ==
(qa (type-of (current-thread)) 'thread)
(qa (type-of (make-mutex)) 'mutex)
(qa (type-of (make-condition-variable (make-mutex))) 'condition-variable)
(qa (type-of (symbol-function 'car)) 'subr)
;; closure type
(qa (type-of (eval '(lambda (x) x) t)) 'interpreted-function)

;; == fboundp on common internals ==
(qa (fboundp 'byte-code-function-p) t)
(qa (fboundp 'compiled-function-p) t)
(qa (fboundp 'native-comp-function-p) t)
(qa (fboundp 'make-mutex) t)
(qa (fboundp 'make-condition-variable) t)
(qa (fboundp 'current-thread) t)
(qa (fboundp 'symbol-file) t)
(qa (fboundp 'permanent-local-variable-p) nil)
;; defsubst / inline are fbound macros
(qa (fboundp 'defsubst) t)
(qa (fboundp 'inline) t)

;; == integer/float predicate micro-coverage (supplement qa_numbers) ==
(qa (zerop 0) t)
(qa (zerop 0.0) t)
(qa (zerop -0.0) t)
(qa-err (zerop "x") 'wrong-type-argument)

(princ (qa--done))
