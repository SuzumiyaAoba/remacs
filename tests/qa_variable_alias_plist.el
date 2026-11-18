;;; -*- lexical-binding: nil; -*-
;; The variable-alias indirection is symbol state, independent of plist edits.
(defvar qa-va-base 42)
(defvar qa-va-other 99)
(defvaralias 'qa-va-alias 'qa-va-base)

(qa (list (eq (indirect-variable 'qa-va-alias) 'qa-va-base)
          (get 'qa-va-alias 'variable-alias)
          (symbol-plist 'qa-va-alias))
    '(t nil (variable-documentation nil)))

(put 'qa-va-alias 'variable-alias 'qa-va-other)
(qa (list (eq (indirect-variable 'qa-va-alias) 'qa-va-base)
          (eq (symbol-value 'qa-va-alias) 42)
          (get 'qa-va-alias 'variable-alias))
    '(t t qa-va-other))

(setplist 'qa-va-alias '(user-property 7))
(qa (list (eq (indirect-variable 'qa-va-alias) 'qa-va-base)
          (eq (symbol-value 'qa-va-alias) 42)
          (symbol-plist 'qa-va-alias))
    '(t t (user-property 7)))

;; Dropping all plist properties must not break cycle checking either.
(qa-err-data (defvaralias 'qa-va-base 'qa-va-alias)
             'cyclic-variable-indirection '(qa-va-alias))

;; A user-created plist property named `variable-alias' remains ordinary data.
(let ((alias 'qa-va-delete))
  (defvaralias alias 'qa-va-base)
  (put alias 'variable-alias 'qa-va-other)
  (internal-delete-indirect-variable alias)
  (qa (list (eq (indirect-variable alias) alias)
            (get alias 'variable-alias)
            (symbol-plist alias)
            (boundp alias))
      '(t qa-va-other (variable-documentation nil variable-alias qa-va-other) nil)))

(princ (qa--done)) (terpri)
