;;; qa_variable_documentation.el --- GNU DOC variable metadata -*- lexical-binding: nil; -*-

;; DOC offsets vary between builds; compare their shape and resolved text.
(dolist (name '(fill-column buffer-read-only inhibit-read-only major-mode))
  (qa (integerp (get name 'variable-documentation)) t)
  (qa (> (get name 'variable-documentation) 0) t)
  (qa (stringp (documentation-property name 'variable-documentation t)) t))

(qa (documentation-property 'buffer-read-only 'variable-documentation t)
    "Non-nil if this buffer is read-only.")
(qa (documentation-property 'fill-column 'variable-documentation t)
    "Column beyond which automatic line-wrapping should happen.
It is used by filling commands, such as `fill-region' and `fill-paragraph',
and by `auto-fill-mode', which see.
See also `current-fill-column'.
Interactively, you can set the buffer local value using \\[set-fill-column].")
(qa (documentation-property 'fill-column 'variable-documentation)
    (substitute-command-keys
     (documentation-property 'fill-column 'variable-documentation t)))
(qa (string-match-p "\\\\\\[set-fill-column\\]"
                    (documentation-property 'fill-column 'variable-documentation)) nil)

;; The global documentation remains the same across buffer-local values.
(qa (with-temp-buffer
      (setq buffer-read-only t)
      (documentation-property 'buffer-read-only 'variable-documentation t))
    "Non-nil if this buffer is read-only.")
;; GUI variables are absent in headless GNU builds.  Where bound, their real
;; DOC entry must survive later SKIP placeholders from other GUI backends.
(when (boundp 'x-use-underline-position-properties)
  (qa (integerp (get 'x-use-underline-position-properties 'variable-documentation)) t)
  (qa (string-prefix-p
       "Non-nil means make use of UNDERLINE_POSITION"
       (documentation-property 'x-use-underline-position-properties
                               'variable-documentation t)) t))

;; A property edit must win over the DOC entry, including an explicit nil.
(let ((saved (get 'fill-column 'variable-documentation)))
  (unwind-protect
      (progn
        (put 'fill-column 'variable-documentation "Replacement doc")
        (qa (documentation-property 'fill-column 'variable-documentation t)
            "Replacement doc")
        (put 'fill-column 'variable-documentation nil)
        (qa (documentation-property 'fill-column 'variable-documentation t) nil)
        (put 'fill-column 'variable-documentation 0)
        (qa (documentation-property 'fill-column 'variable-documentation t) nil))
    (put 'fill-column 'variable-documentation saved)))
(defvaralias 'qa-doc-fill-alias 'fill-column)
(qa (documentation-property 'qa-doc-fill-alias 'variable-documentation t)
    (documentation-property 'fill-column 'variable-documentation t))
(defvar qa-doc-user-variable 1 "User variable documentation")
(qa (get 'qa-doc-user-variable 'variable-documentation)
    "User variable documentation")
(qa (documentation-property 'qa-doc-unknown-variable 'variable-documentation) nil)

(require 'cl-lib)
(qa (type-of (symbol-function 'car)) 'subr)
(qa (cl-type-of (symbol-function 'car)) 'primitive-function)
(qa (cl-type-of (symbol-function 'if)) 'special-form)
(qa (cl-type-of (symbol-function 'let)) 'special-form)
(describe-function 'car)
(qa (with-current-buffer "*Help*"
      (not (null (string-match-p "car is a primitive-function in"
                                (buffer-string))))) t)
(describe-variable 'fill-column)
(qa (with-current-buffer "*Help*"
      (not (null (string-match-p "fill-column is a variable defined in"
                                (buffer-string))))) t)
(qa (with-current-buffer "*Help*"
      (not (null (string-match-p "C source code" (buffer-string))))) t)
(qa (with-current-buffer "*Help*"
      (not (null (string-match-p "Column beyond which automatic line-wrapping"
                                (buffer-string))))) t)
(qa (with-current-buffer "*Help*"
      (string-match-p "Not documented as a variable" (buffer-string))) nil)
(princ (qa--done))
