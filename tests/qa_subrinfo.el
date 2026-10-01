;; subr introspection — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - subr-name / subr-arity take the FUNCTION (not a symbol).
;;   - subr-arity returns (MIN . MAX-or-'many).
;;   - special-form-p is t for special forms only.
;;   - macrop is t for Lisp macros.
;;   - subrp on a subr → t; on a symbol → nil (not a subr).
;;   - functionp on a lambda list → t (it IS callable).
;;   - readablep returns the prin1 STRING ("42" for 42) when the
;;     object has read syntax; nil for unreadable objects (subrs,
;;     buffers).
;;   - featurep 'cl-lib is nil in -Q batch (cl-lib not preloaded).
;;
;; NOTE: `qa' evals each form dynamically — bind locals INSIDE the
;; checked form (a lexical `let' outside `qa' is invisible to eval).

;; == surface ==
(qa (fboundp 'subr-name) t)
(qa (fboundp 'subr-arity) t)
(qa (fboundp 'subrp) t)
(qa (fboundp 'special-form-p) t)
(qa (fboundp 'macrop) t)
(qa (fboundp 'functionp) t)
(qa (fboundp 'byte-code-function-p) t)
(qa (fboundp 'compiled-function-p) t)
(qa (fboundp 'indirect-function) t)
(qa (fboundp 'fboundp) t)
(qa (fboundp 'fset) t)
(qa (fboundp 'fmakunbound) t)
(qa (fboundp 'defalias) t)
(qa (fboundp 'commandp) t)
(qa (fboundp 'interactive-form) t)
(qa (fboundp 'autoloadp) t)
(qa (fboundp 'readablep) t)

;; == subr-name on the function cell ==
(qa (subr-name (symbol-function 'car)) "car")
(qa (subr-name (symbol-function '+)) "+")

;; == subr-arity returns (MIN . MAX) ==
(qa (subr-arity (symbol-function 'car)) '(1 . 1))
;; variadic
(qa (subr-arity (symbol-function '+)) '(0 . many))
;; &optional
(qa (subr-arity (symbol-function 'string-match)) '(2 . 4))

;; == subrp ==
(qa (subrp (symbol-function 'car)) t)
(qa (subrp (symbol-function '+)) t)
;; a symbol is not a subr
(qa (subrp 'car) nil)
(qa (subrp nil) nil)
;; a lambda is not a subr
(qa (subrp (lambda (x) x)) nil)

;; == special-form-p ==
(qa (special-form-p 'if) t)
(qa (special-form-p 'let) t)
(qa (special-form-p 'while) t)
(qa (special-form-p 'progn) t)
(qa (special-form-p 'quote) t)
;; car is a subr, not a special form
(qa (special-form-p 'car) nil)
;; user-defined macros are not special forms
(qa (special-form-p 'when) nil)

;; == macrop on Lisp macros ==
(qa (macrop 'when) t)
(qa (macrop 'unless) t)
;; car is a subr, not a macro
(qa (macrop 'car) nil)
(qa (macrop 'if) nil)

;; == functionp ==
(qa (functionp 'car) t)
(qa (functionp (lambda (x) x)) t)
(qa (functionp #'car) t)
;; a lambda-list is a valid function in GNU → t
(qa (functionp '(lambda (x) x)) t)
;; not functions
(qa (functionp 'nil) nil)
(qa (functionp 42) nil)
(qa (functionp "x") nil)

;; == byte-code-function-p ==
(qa (byte-code-function-p (symbol-function 'car)) nil)
(qa (byte-code-function-p (lambda (x) x)) nil)

;; == readablep ==
;; returns the PRINTED REPRESENTATION string, not the object
;; remacs: returns the object itself
(qa (readablep 42) "42")
(qa (readablep "s") "\"s\"")
(qa (readablep 'a) "a")
;; closures print readably
(qa (stringp (readablep (lambda (x) x))) t)
;; subrs have no read syntax → nil
(qa (readablep (symbol-function 'car)) nil)
;; buffers are unreadable → nil
(qa (readablep (current-buffer)) nil)

;; == indirect-function ==
(qa (indirect-function 'car) (symbol-function 'car))
;; follows aliases
(qa (progn
      (defalias 'qa-si-alias #'car)
      (prog1 (indirect-function 'qa-si-alias)
        (fmakunbound 'qa-si-alias)))
    (symbol-function 'car))
;; unbound symbol → nil
(qa (indirect-function 'undefined-xyz) nil)

;; == fboundp on symbols vs others ==
(qa (fboundp 'car) t)
(qa (fboundp 'when) t)
(qa (fboundp 'undefined-xyz-qa) nil)

;; == fmakunbound ==
(qa (progn
      (fset 'qa-si-tmp #'car)
      (prog1 (fboundp 'qa-si-tmp)
        (fmakunbound 'qa-si-tmp)))
    t)
(qa (fboundp 'qa-si-tmp) nil)

;; == defalias ==
(qa (progn
      (defalias 'qa-si-plus #'+)
      (prog1 (funcall 'qa-si-plus 1 2)
        (fmakunbound 'qa-si-plus)))
    3)

;; == commandp on subr / lambda / interactive ==
(qa (commandp 'self-insert-command) t)
;; non-interactive subr
(qa (commandp 'car) nil)
;; lambda without (interactive) → nil
(qa (commandp (lambda (x) x)) nil)
;; lambda with (interactive) → t
(qa (commandp (lambda (x) (interactive) x)) t)
;; keyboard macro strings are commands
(qa (commandp "abc") t)
;; vectors of events too
(qa (commandp [?a ?b]) t)

;; interactive-form can hang in batch — skip
;; autoloadp on a non-autoloaded symbol → nil
(qa (autoloadp (symbol-function 'car)) nil)

;; == featurep on core features ==
;; cl-lib IS loaded by the time we get here: the Missing
;; `lexical-binding' warning machinery pulls it in on the first
;; `load'. So featurep reports t, not nil.
(qa (featurep 'cl-lib) t)
(qa (featurep 'undefined-feature-xyz) nil)
;; emacs itself is a feature
(qa (featurep 'emacs) t)

;; == provide — adds to features ==
(qa (progn (provide 'qa-test-feature)
           (featurep 'qa-test-feature))
    t)

;; == obarray predicates / intern machinery ==
(qa (boundp 'obarrayp) nil)
(qa (functionp #'intern) t)
(qa (functionp #'intern-soft) t)
(qa (functionp #'unintern) t)
(qa (functionp #'mapatoms) t)

;; == intern / intern-soft round trip ==
(qa (let ((s (intern "qa-test-sym-xyz")))
      (prog1
          (list (symbolp s)
                (symbol-name s)
                ;; intern-soft finds it
                (eq (intern-soft "qa-test-sym-xyz") s))
        (unintern 'qa-test-sym-xyz nil)))
    '(t "qa-test-sym-xyz" t))
;; gone after unintern
(qa (intern-soft "qa-test-sym-xyz") nil)

;; == intern-soft on existing symbols ==
(qa (eq (intern-soft "car") 'car) t)

(princ (qa--done))
