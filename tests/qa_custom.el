;; customize — defgroup/defcustom/defface/obsolete-alias machinery.
;; Missing in GNU 31.1 batch (skipped, not tested): `user-variable-p',
;; `custom-face-p', `face-spec-get', `face-remapping-base',
;; `internal-facep' (all void functions).
(require 'cus-edit) ;; custom-group-members/custom-theme-p live in cus-edit

;; == defgroup ==
(qa (defgroup qacu-g nil "A test group.") 'qacu-g)
(qa (get 'qacu-g 'group-documentation) "A test group.")
(qa (custom-group-members 'qacu-g nil) nil)
(qa (get 'qacu-no-such-group 'group-documentation) nil)

;; == defcustom core ==
(qa (defcustom qacu-v 5 "Doc for v."
      :type 'integer :group 'qacu-g :safe 'integerp)
    'qacu-v)
(qa qacu-v 5)
(qa (symbol-value 'qacu-v) 5)
(qa (default-value 'qacu-v) 5)
;; custom-variable-p returns the standard-value list, not t.
;; This file has no lexical-binding cookie, so defcustom stores the
;; quoted STANDARD form — GNU's car is the raw expression 5.
(qa (length (custom-variable-p 'qacu-v)) 1)
(qa (car (custom-variable-p 'qacu-v)) 5)
(qa (length (get 'qacu-v 'standard-value)) 1)
;; the stored standard value is an evaluated funcall thunk → eval → 5
(qa (eval (car (get 'qacu-v 'standard-value)) t) 5)
(qa (custom-variable-p 'car) nil)
(qa (custom-variable-p nil) nil)
(qa (custom-variable-p 'qacu-not-defined) nil)
(qa (get 'qacu-v 'safe-local-variable) 'integerp)
(qa (get 'qacu-v 'custom-type) 'integer)
(qa (get 'qacu-v 'custom-group) nil)     ; membership lives on the group
(qa (get 'qacu-v 'custom-version) nil)
(qa (get 'qacu-v 'variable-documentation) "Doc for v.")
(qa (safe-local-variable-p 'qacu-v 99) t)
(qa (safe-local-variable-p 'qacu-v "x") nil)
;; group membership recorded on the GROUP symbol via defcustom :group.
;; Note: a defcustom WITHOUT :group joins the last group defined so far.
(qa (custom-group-members 'qacu-g nil) '((qacu-v custom-variable)))
;; recursive arg lists only SUBGROUP members, not the group's own
(qa (custom-group-members 'qacu-g t) nil)

;; == defcustom options :risky/:version ==
(defcustom qacu-risky "/tmp/x" "doc" :type 'string :risky t)
(qa (get 'qacu-risky 'risky-local-variable) t)
(defcustom qacu-vers 1 "doc" :version "31.1")
(qa (get 'qacu-vers 'custom-version) "31.1")
;; a value violating :type is still accepted at define time
(defcustom qacu-bad "x" "doc" :type 'integer)
(qa (get 'qacu-bad 'custom-type) 'integer)
(qa qacu-bad "x")
(defcustom qacu-plain-var 'z "plain" :type 'symbol)
(qa (safe-local-variable-p 'qacu-plain-var 'z) nil)
(qa (custom-group-members 'qacu-g nil)
    '((qacu-v custom-variable) (qacu-risky custom-variable)
      (qacu-vers custom-variable) (qacu-bad custom-variable)
      (qacu-plain-var custom-variable)))

;; == :set is invoked at defcustom time; :get is for the UI only ==
(defvar qacu-log nil)
(defcustom qacu-sg 10 "doc"
  :type 'integer
  :set (lambda (sym val)
         (push (list 'set sym val) qacu-log)
         (set-default sym (* val 2)))
  :get (lambda (sym) (* 100 (default-value sym))))
;; setter ran once with the default → 10*2 = 20; :get not consulted
(qa qacu-sg 20)
(qa (symbol-value 'qacu-sg) 20)
(qa qacu-log '((set qacu-sg 10)))
;; plain setq bypasses :set entirely
(setq qacu-sg 77)
(qa qacu-sg 77)
(qa (length qacu-log) 1)

;; == custom-set-* ==
(qa (custom-set-default 'qacu-v 7) nil)
(qa qacu-v 7)
(qa (default-value 'qacu-v) 7)
;; in batch, custom-set-variables applies immediately (no :now needed)
(qa (custom-set-variables '(qacu-v 9)) nil)
(qa qacu-v 9)
(qa (default-value 'qacu-v) 9)
;; :now t runs the :set function → setter doubles, logs
(qa (custom-set-variables '(qacu-sg 3 t)) nil)
(qa qacu-sg 6)
(qa qacu-log '((set qacu-sg 3) (set qacu-sg 10)))
;; custom-reevaluate-setting returns the CURRENT value here
(qa (custom-reevaluate-setting 'qacu-v) 9)
(qa qacu-v 9)

;; == custom-add-to-group / member ordering ==
(defcustom qacu-v2 'y "doc" :type 'symbol)
;; qacu-v2 already auto-joined qacu-g (the last group defined);
;; custom-add-to-group with the same group does not duplicate it
(qa (custom-add-to-group 'qacu-g 'qacu-v2 'custom-variable) nil)
(qa (get 'qacu-v2 'custom-group) nil)
;; nested group appears as (name custom-group) in the parent
(defgroup qacu-sub nil "sub group" :group 'qacu-g)
(qa (get 'qacu-sub 'custom-group) nil)
(qa (custom-group-members 'qacu-sub nil) nil)
(defgroup qacu-vg nil "v" :version "31.1" :group 'qacu-g)
(qa (get 'qacu-vg 'custom-version) "31.1")

;; the full member list: auto-joined defcustoms, explicit add, subgroups
(qa (custom-group-members 'qacu-g nil)
    '((qacu-v custom-variable) (qacu-risky custom-variable)
      (qacu-vers custom-variable) (qacu-bad custom-variable)
      (qacu-plain-var custom-variable) (qacu-sg custom-variable)
      (qacu-v2 custom-variable) (qacu-sub custom-group)
      (qacu-vg custom-group)))

;; == defface ==
(qa (defface qacu-f '((t (:foreground "red" :weight bold)))
      "Face doc." :group 'qacu-g)
    'qacu-f)
(qa (vectorp (facep 'qacu-f)) t)         ; facep returns an attr vector
(qa (facep 'qacu-noface) nil)
(qa (facep "x") nil)
(qa (vectorp (facep 'default)) t)
(qa (face-attribute 'qacu-f :foreground) "red")
(qa (face-attribute 'qacu-f :foreground nil 'default) "red")
(qa (face-attribute 'qacu-f :weight) 'bold)
(qa (face-attribute 'qacu-f :slant) 'unspecified)
(qa (face-attribute 'qacu-f :inherit) 'unspecified)
(qa (face-attribute 'default :foreground) "unspecified-fg")
(qa (face-attribute 'default :foreground nil 'default) "unspecified-fg")
(qa (face-documentation 'qacu-f) "Face doc.")
(qa (face-documentation 'default) "Basic default face.")
(qa (face-documentation 'qacu-noface) nil)
(qa (face-id 'default) 0)
(qa (integerp (face-id 'qacu-f)) t)
;; 'default is the last element of face-list
(qa (memq 'default (face-list)) '(default))
(qa (not (null (memq 'qacu-f (face-list)))) t)
(qa (get 'qacu-f 'face-defface-spec) '((t (:foreground "red" :weight bold))))
;; the defface :group lands on qacu-g even though qacu-vg came later
(qa (car (last (custom-group-members 'qacu-g nil)))
    '(qacu-f custom-face))
;; face-bold-p returns the memq tail of bold weights, not t
(qa (face-bold-p 'qacu-f) '(bold extra-bold ultra-bold))
(qa (face-italic-p 'qacu-f) nil)
(qa (face-underline-p 'qacu-f) nil)
(qa (face-inverse-video-p 'qacu-f) nil)
(qa (face-attribute-relative-p 'qacu-f :weight) nil)
(qa (face-equal 'qacu-f 'qacu-f) t)
(qa (face-equal 'qacu-f 'default) nil)
;; batch quirk: differs-from-default-p can't tell on the batch terminal
(qa (face-differs-from-default-p 'qacu-f) nil)
(qa (face-differs-from-default-p 'default) nil)
(qa (assq :foreground (face-all-attributes 'default))
    '(:foreground . unspecified))

;; == face errors ==
(qa-err (face-attribute 'qacu-nosuchface :foreground) 'error)
(qa-err (face-attribute 'default :bogus) 'error)

;; == :inherit on faces is stored, not resolved by face-attribute ==
(defface qacu-ch '((t (:inherit qacu-f))) "child face")
(qa (face-attribute 'qacu-ch :inherit) 'qacu-f)
(qa (face-attribute 'qacu-ch :foreground) 'unspecified)
;; :inherit IS followed when merging toward an inherit arg / 'default
(qa (face-attribute 'qacu-ch :foreground nil 'default) "red")
(qa (face-attribute 'qacu-ch :foreground nil t) "red")

;; == set-face-attribute mutates runtime attrs, not the defface spec ==
(qa (set-face-attribute 'qacu-f nil :foreground "blue" :slant 'italic) nil)
(qa (face-attribute 'qacu-f :foreground) "blue")
(qa (face-attribute 'qacu-f :slant) 'italic)
(qa (get 'qacu-f 'face-defface-spec) '((t (:foreground "red" :weight bold))))
(qa (set-face-attribute 'qacu-f nil :foreground 'unspecified) nil)
(qa (face-attribute 'qacu-f :foreground) 'unspecified)
;; reset for reproducibility
(set-face-attribute 'qacu-f nil :foreground "red" :slant 'unspecified)

;; == make-empty-face / make-face / copy-face ==
(qa (make-empty-face 'qacu-ef) 'qacu-ef)
(qa (vectorp (facep 'qacu-ef)) t)
(qa (make-face 'qacu-mf) 'qacu-mf)
(qa (vectorp (facep 'qacu-mf)) t)
(qa (copy-face 'qacu-f 'qacu-f2) 'qacu-f2)
(qa (vectorp (facep 'qacu-f2)) t)
(qa (face-equal 'qacu-f 'qacu-f2) t)

;; == obsolete machinery ==
;; define-obsolete-variable-alias → byte-obsolete-variable prop
(qa (define-obsolete-variable-alias 'qacu-old 'qacu-v "31.1") 'qacu-old)
(qa (eq (indirect-variable 'qacu-old) 'qacu-v) t)
(qa (boundp 'qacu-old) t)
(qa (symbol-value 'qacu-old) 9)
(qa (get 'qacu-old 'byte-obsolete-variable) '(qacu-v nil "31.1"))
;; note: no plain 'obsolete prop is installed
(qa (get 'qacu-old 'obsolete) nil)
;; define-obsolete-function-alias → byte-obsolete-info prop
(qa (define-obsolete-function-alias 'qacu-ofn #'1+ "31.1") 'qacu-ofn)
(qa (qacu-ofn 5) 6)
(qa (eq (indirect-function 'qacu-ofn) (symbol-function '1+)) t)
(qa (get 'qacu-ofn 'byte-obsolete-info) '(1+ nil "31.1"))
(qa (get 'qacu-ofn 'obsolete) nil)
;; make-obsolete on a FUNCTION puts byte-obsolete-info...
(defun qacu-realfn (x) (1+ x))
(make-obsolete 'qacu-realfn 'qacu-new "31.1")
(qa (get 'qacu-realfn 'byte-obsolete-info) '(qacu-new nil "31.1"))
;; ...but on a symbol with no function cell it still writes
;; byte-obsolete-info (not byte-obsolete-variable!)
(defvar qacu-ov 1)
(make-obsolete 'qacu-ov nil "31.1")
(qa (get 'qacu-ov 'byte-obsolete-info) '(nil nil "31.1"))
(qa (get 'qacu-ov 'byte-obsolete-variable) nil)
;; make-obsolete-variable writes byte-obsolete-variable
(defvar qacu-ov2 2)
(make-obsolete-variable 'qacu-ov2 'qacu-new "31.1")
(qa (get 'qacu-ov2 'byte-obsolete-variable) '(qacu-new nil "31.1"))
;; WHEN argument is mandatory in 31.1
(qa-err (make-obsolete 'qacu-x "use y") 'wrong-number-of-arguments)
;; define-obsolete-face-alias → face-alias + obsolete-face props
;; returns the WHEN string, not the symbol
(qa (define-obsolete-face-alias 'qacu-oface 'default "31.1") "31.1")
(qa (get 'qacu-oface 'face-alias) 'default)
(qa (get 'qacu-oface 'obsolete-face) "31.1")

;; == defvaralias ==
(defvar qacu-base 11 "base doc")
(qa (defvaralias 'qacu-al 'qacu-base) 'qacu-base)
(qa (eq (indirect-variable 'qacu-al) 'qacu-base) t)
(qa qacu-al 11)
;; setq through the alias writes through to the base variable
(setq qacu-al 99)
(qa qacu-base 99)
(qa (get 'qacu-al 'variable-documentation) nil)
(qa (get 'qacu-base 'variable-documentation) "base doc")
(defvaralias 'qacu-al2 'qacu-al "Alias doc.")
(qa (get 'qacu-al2 'variable-documentation) "Alias doc.")
;; aliases chain through to the base
(qa (indirect-variable 'qacu-al2) 'qacu-base)
;; indirect-variable on a plain/unbound symbol returns it unchanged
(qa (indirect-variable 'qacu-v) 'qacu-v)
(qa (indirect-variable 'qacu-nothere) 'qacu-nothere)
(defalias 'qacu-fa #'car)
(qa (eq (indirect-function 'qacu-fa) (symbol-function 'car)) t)

;; == declare-function ==
(qa (declare-function qacu-ext "somewhere" (a b)) nil)

;; == themes (no enabling/persistence) ==
(qa (deftheme qacu-th "test theme") nil)
;; custom-theme-p returns the memq tail of custom-known-themes
(qa (custom-theme-p 'qacu-th) '(qacu-th user changed))
(qa (custom-theme-p 'user) '(user changed))
(qa (custom-theme-p 'qacu-notheme) nil)
(qa (get 'qacu-th 'theme-documentation) "test theme")
(qa (not (null (memq 'wombat (custom-available-themes)))) t)

;; == misc ==
;; custom-quote is a plain function, not a macro
(qa (macroexpand '(custom-quote 5)) '(custom-quote 5))
(qa (fboundp 'custom-declare-variable) t)
(qa (fboundp 'custom-declare-group) t)
(qa (fboundp 'custom-declare-face) t)
(qa (fboundp 'custom-add-to-group) t)
(qa (fboundp 'custom-add-load) t)
(qa (fboundp 'custom-push-theme) t)
(qa (fboundp 'custom-load-symbol) t)
(qa (fboundp 'custom-variable-documentation) t)
(qa (fboundp 'custom-buffer-create) t)
(qa (fboundp 'widget-create) t)
;; absent in GNU 31.1 batch:
(qa (fboundp 'user-variable-p) nil)
(qa (fboundp 'custom-face-p) nil)
(qa (fboundp 'face-remapping-base) nil)
(qa (fboundp 'internal-facep) nil)
(qa (fboundp 'face-spec-get) nil)

(princ (qa--done))
