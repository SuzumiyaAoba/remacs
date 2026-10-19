;;; -*- lexical-binding: t; -*-
;; Issue #98: constants cannot become buffer-local; C-backed variables
;; cannot be redefined as aliases. Failed operations leave bindings intact.
(defvar qa98-variable)
(defvar qa98-operation)
(dolist (qa98-variable '(nil t :qa98-constant))
  (dolist (qa98-operation '(make-local-variable make-variable-buffer-local))
    (qa-err-data (funcall qa98-operation qa98-variable)
                 'setting-constant (list qa98-variable))
    (qa (local-variable-p qa98-variable) nil)
    (qa (local-variable-if-set-p qa98-variable) nil))
  (qa-err-data (defvaralias qa98-variable 'qa98-unbound-base)
               'error (list (format "Cannot make a constant an alias: %s" qa98-variable))))
(qa (boundp 'qa98-unbound-base) nil)
(qa-err-data (defvaralias nil 1) 'wrong-type-argument '(symbolp 1))
(qa-err-data (make-local-variable 1) 'wrong-type-argument '(symbolp 1))
(qa-err-data (make-variable-buffer-local "x") 'wrong-type-argument '(symbolp "x"))

;; A constant's alias inherits its protection, including through a chain.
(defvaralias 'qa98-nil nil)
(defvaralias 'qa98-nil2 'qa98-nil)
(defvaralias 'qa98-true t)
(defvaralias 'qa98-keyword :qa98-constant)
(dolist (qa98-variable '(qa98-nil qa98-nil2 qa98-true qa98-keyword))
  (dolist (qa98-operation '(make-local-variable make-variable-buffer-local))
    (qa-err-data (funcall qa98-operation qa98-variable)
                 'setting-constant (list qa98-variable)))
  (qa-err-data (defvaralias qa98-variable 'qa98-unbound-base)
               'error (list (format "Cannot make a constant an alias: %s" qa98-variable))))

(dolist (qa98-variable '(fill-column tab-width major-mode buffer-file-name
                        ctl-arrow gc-cons-threshold read-circle standard-output
                        last-command-event load-path features))
  (qa (let* ((before (symbol-value qa98-variable))
             (documentation (get qa98-variable 'variable-documentation))
             (base (make-symbol "qa98-base"))
             (result (condition-case error (defvaralias qa98-variable base) (error error))))
        (list result (eq (indirect-variable qa98-variable) qa98-variable)
              (eq (symbol-value qa98-variable) before) (boundp base)
              (eq (get qa98-variable 'variable-documentation) documentation)))
      (list (list 'error (format "Cannot make a built-in variable an alias: %s" qa98-variable))
            t t nil t)))
;; Cycle checking precedes the built-in check, after constant/type checks.
(qa-err-data (defvaralias 'fill-column 'fill-column)
             'cyclic-variable-indirection '(fill-column))
(qa-err-data (defvaralias 'fill-column 1) 'wrong-type-argument '(symbolp 1))

;; Lisp defconst declarations remain mutable. Uninterned colon-prefixed
;; symbols are ordinary variables, unlike interned keywords.
(defconst qa98-lisp-constant 10)
(qa (with-temp-buffer
      (make-local-variable 'qa98-lisp-constant)
      (setq qa98-lisp-constant 20)
      (list qa98-lisp-constant (default-value 'qa98-lisp-constant))) '(20 10))
(qa (with-temp-buffer
      (let ((variable (make-symbol ":qa98-ordinary")))
        (set variable 12)
        (list (eq (make-local-variable variable) variable)
              (local-variable-p variable) (symbol-value variable)))) '(t t 12))
(qa (let ((alias (make-symbol "qa98-alias")) (base (make-symbol "qa98-base")))
      (set base 10)
      (defvaralias alias base)
      (with-temp-buffer
        (list (eq (make-local-variable alias) base)
              (local-variable-p alias) (local-variable-p base)
              (progn (set alias 20) (buffer-local-value alias (current-buffer)))
              (default-value base)
              (progn (kill-local-variable alias) (symbol-value alias))
              (local-variable-p base)))) '(t t t 20 10 10 nil))
(qa (let ((alias (make-symbol "qa98-alias")) (base (make-symbol "qa98-base")))
      (set base 10) (defvaralias alias base)
      (list (eq (make-variable-buffer-local alias) alias)
            (local-variable-if-set-p base)
            (with-temp-buffer (set alias 20)
              (list (symbol-value base) (buffer-local-boundp alias (current-buffer))))
            (default-value base))) '(t t (20 t) 10))

;; C-backed buffer slots stay forwarded when localized. A global C
;; variable instead becomes a localized C-backed cell, even after killing
;; its concrete local binding; both states remain protected from aliases.
(defvaralias 'qa98-fill 'fill-column)
(qa (with-temp-buffer
      (list (make-local-variable 'qa98-fill)
            (condition-case error (defvaralias 'fill-column 'qa98-unbound-base) (error error))))
    '(qa98-fill (error "Cannot make a built-in variable an alias: fill-column")))
(qa (with-temp-buffer
      (make-local-variable 'read-circle)
      (kill-local-variable 'read-circle)
      (condition-case error (defvaralias 'read-circle 'qa98-unbound-base) (error error)))
    '(error "Don’t know how to make a buffer-local variable an alias: read-circle"))
(qa (progn (make-variable-buffer-local 'gc-cons-threshold)
           (condition-case error (defvaralias 'gc-cons-threshold 'qa98-unbound-base) (error error)))
    '(error "Don’t know how to make a buffer-local variable an alias: gc-cons-threshold"))

(princ (qa--done)) (terpri)
