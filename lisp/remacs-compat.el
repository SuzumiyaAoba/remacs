;;; remacs-compat.el --- fallbacks for unimplemented primitives -*- lexical-binding: t; -*-

;; remacs: definitions for primitives/macros that are registered as
;; stubs but not yet implemented by the runtime.  Each definition is
;; guarded so it becomes a no-op once the real implementation exists.
;;
;; Load with (require 'remacs-compat) at the top of libraries whose
;; expansions (cl-defun/cl-defgeneric/oclosure-define/defcustom-derived
;; forms) call these primitives.

;;; Code:

;; `set-advertised-calling-convention' is emitted by cl-defun /
;; cl-defgeneric / defalias-family expansions (see `byte-run.el').
(unless (fboundp 'set-advertised-calling-convention)
  (defun set-advertised-calling-convention (function signature _when)
    "Set the advertised SIGNATURE of FUNCTION (remacs subset)."
    (put function 'advertised-calling-convention signature)))

;; `cl--find-class' is used by cl-defstruct/oclosure/eieio expansions
;; both as a function and as a `setf' target; GNU stores the class on
;; the `cl--class' symbol property.
(unless (symbol-function 'cl--find-class)
  (defun cl--find-class (name)
    "Return the class of type NAME (remacs subset)."
    (get name 'cl--class))
  (gv-define-setter cl--find-class (store name)
    `(put ,name 'cl--class ,store)))

;; `define-obsolete-function-alias' / `define-obsolete-variable-alias'
;; are used by obsolescence declarations; record what GNU records.
(unless (fboundp 'define-obsolete-function-alias)
  (defmacro define-obsolete-function-alias (obsolete-name current-name
                                            &optional when docstring)
    "Define OBSOLETE-NAME as an obsolete alias for CURRENT-NAME (subset)."
    (declare (doc-string 4))
    `(progn
       (defalias ,obsolete-name ,current-name ,docstring)
       (put ,obsolete-name 'byte-obsolete-info
            (list ,current-name nil ,when)))))

(unless (fboundp 'define-obsolete-variable-alias)
  (defmacro define-obsolete-variable-alias (obsolete-name current-name
                                            &optional when docstring)
    "Define OBSOLETE-NAME as an obsolete alias for CURRENT-NAME (subset)."
    (declare (doc-string 4))
    `(progn
       (defvaralias ,obsolete-name ,current-name ,docstring)
       (put ,obsolete-name 'byte-obsolete-variable
            (list ,current-name nil ,when)))))

;; `byte-run--set-speed' is the `declare' handler for (speed N); GNU's
;; version returns a form which the declaration machinery evaluates.
(unless (fboundp 'byte-run--set-speed)
  (defalias 'byte-run--set-speed
    (lambda (f _args val)
      (list 'function-put (list 'quote f) ''speed (list 'quote val)))))

;; `set-face-documentation' records a face's docstring on the
;; `face-documentation' property, as GNU does.
(unless (fboundp 'set-face-documentation)
  (defun set-face-documentation (face string)
    "Set the documentation string for FACE to STRING."
    ;; Perhaps the text should go in DOC.
    (put face 'face-documentation string)))

;; Variables defined in prelude sections the runtime may skip; these
;; are boundp-guarded so the real definitions win whenever present.
(unless (boundp 'emacs-lisp-mode-syntax-table)
  (defvar emacs-lisp-mode-syntax-table
    (let ((table (make-syntax-table)))
      (modify-syntax-entry ?@ "_" table)
      table)
    "Syntax table used in `emacs-lisp-mode' (remacs subset)."))

(unless (boundp 'special-mode-map)
  (defvar special-mode-map (make-sparse-keymap "Special")
    "Parent keymap for special modes (remacs subset)."))

;; `occur-mode-map' (loaded earlier than its menu map in the merged
;; subr-x order) references `occur-menu-map'; GNU defines it first via
;; replace.el.  `defvar' there keeps this initial value.
(unless (boundp 'occur-menu-map)
  (defvar occur-menu-map (make-sparse-keymap "Occur")
    "Menu keymap for `occur-mode' (remacs subset)."))

;; `ignored-local-variables' is a files.el defvar whose value is
;; extended by connection-local variables support.
(unless (boundp 'ignored-local-variables)
  (defvar ignored-local-variables
    '(ignored-local-variables safe-local-variable-values
      file-local-variables-alist dir-local-variables-alist)
    "Variables to be ignored in a file's local variable spec.")
  (put 'ignored-local-variables 'risky-local-variable t))

;; `face-attribute-name-alist' is consulted by the defface machinery at
;; load time; identical to the faces.el defconst.
(unless (boundp 'face-attribute-name-alist)
  (defconst face-attribute-name-alist
    '((:family . "font family")
      (:foundry . "font foundry")
      (:width . "character set width")
      (:height . "height in 1/10 pt")
      (:weight . "weight")
      (:slant . "slant")
      (:underline . "underline")
      (:overline . "overline")
      (:extend . "extend")
      (:strike-through . "strike-through")
      (:box . "box")
      (:inverse-video . "inverse-video display")
      (:foreground . "foreground color")
      (:background . "background color")
      (:stipple . "background stipple")
      (:inherit . "inheritance"))
    "An alist of descriptive names for face attributes."))

;; `define-abbrev-table' creates an abbrev-table variable and installs
;; DEFINITIONS ((NAME EXPANSION ...) elements).  GNU stores expansions
;; in the value cell of symbols interned in the table obarray; the
;; subset below does the same and keeps the docstring property.
(unless (fboundp 'define-abbrev-table)
  (defun define-abbrev-table (tablename definitions &optional docstring
                                        &rest _props)
    "Define TABLENAME as an abbrev table name (remacs subset)."
    (let ((table (or (bound-and-true-p tablename) (make-abbrev-table))))
      (dolist (def definitions)
        (set (intern (car def) table) (cadr def)))
      (set tablename table)
      (when docstring
        (put tablename 'variable-documentation docstring))
      table)))

;; `obarray-put'/`obarray-get' are thin wrappers around interning in an
;; obarray, exactly as GNU defines them.
(unless (fboundp 'obarray-put)
  (defun obarray-put (ob name)
    "Return symbol named NAME from obarray OB, creating it if needed."
    (intern name ob)))

(unless (fboundp 'obarray-get)
  (defun obarray-get (ob name)
    "Return symbol named NAME if contained in obarray OB, else nil."
    (intern-soft name ob)))

;; `define-button-type'/`button-category-symbol' provide button.el's
;; type machinery: a per-type hidden symbol carrying the property
;; defaults.  The subset keeps GNU's property layout.
(unless (fboundp 'button-category-symbol)
  (defun button-category-symbol (type)
    "Return the symbol used by button-type TYPE to store properties."
    (or (get type 'button-category-symbol) type)))

(unless (fboundp 'define-button-type)
  (defun define-button-type (name &rest properties)
    "Define a `button type' called NAME (remacs subset)."
    (declare (indent defun))
    (let ((catsym (make-symbol (concat (symbol-name name) "-button")))
          (super-catsym
           (button-category-symbol
            (or (plist-get properties 'supertype)
                (plist-get properties :supertype)
                'button))))
      (put name 'button-category-symbol catsym)
      (let ((default-props (symbol-plist super-catsym)))
        (while default-props
          (put catsym (pop default-props) (pop default-props))))
      (put catsym 'type name)
      (while properties
        (put catsym (pop properties) (pop properties)))
      name)))

;; `function-get' is used by gv/defsetf machinery to read properties
;; along symbol aliases; fall back to a plain `get'.
(unless (fboundp 'function-get)
  (defun function-get (function prop &optional autoload)
    "Return the value of FUNCTION's PROP property (remacs subset)."
    (let ((val (get function prop)))
      (when (and autoload (eq 'autoload (car-safe val)))
        val)
      val)))

;; `easy-menu-define' is called at top level by merged library files;
;; the real implementation is in easymenu.
(unless (fboundp 'easy-menu-define)
  (require 'easymenu))

(provide 'remacs-compat)

;;; remacs-compat.el ends here
