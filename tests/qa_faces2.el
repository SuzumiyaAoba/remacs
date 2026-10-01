;; faces & colors II — attribute plumbing, remapping, face internals,
;; and the tty color database.  Complements qa_custom.el, which covers
;; defface/face-attribute/facep/face-equal basics.
;; All values pinned to GNU Emacs 31.1 -Q --batch; batch quirks are
;; noted in comments.
(require 'face-remap)  ; face-remap-remove-relative lives here, not autoloaded

;; == test faces ==
(qa (defface qaf2-f '((t (:foreground "red" :background "blue"
                         :weight bold :slant italic)))
      "qaf2 doc.") 'qaf2-f)
(defface qaf2-ch '((t (:inherit qaf2-f))) "child of qaf2-f")
(defface qaf2-inv '((t (:foreground "black" :background "white"))) "inv")
(defface qaf2-iv '((t (:inverse-video t))) "iv")
(defface qaf2-ff '((t (:font "fixed"))) "font face")
(define-obsolete-face-alias 'qaf2-alias 'qaf2-f "31.1")

;; == face-foreground / face-background / face-font ==
(qa (face-foreground 'qaf2-f) "red")
(qa (face-background 'qaf2-f) "blue")
(qa (face-foreground 'qaf2-f nil) "red")
(qa (face-foreground 'qaf2-f (selected-frame)) "red")
;; INHERIT arg follows the :inherit chain to find a value
(qa (face-foreground 'qaf2-f nil t) "red")
;; no fg stored anywhere and INHERIT arg absent → nil, not 'unspecified
(qa (face-foreground 'qaf2-ch) nil)
(qa (face-foreground 'qaf2-ch nil t) "red")
;; batch terminal has no real colors: default reports pseudo-strings
(qa (face-foreground 'default) "unspecified-fg")
(qa (face-background 'default) "unspecified-bg")
;; pseudo-strings survive even with INHERIT=t
(qa (face-foreground 'default nil t) "unspecified-fg")
(qa (face-background 'default nil t) "unspecified-bg")
(qa-err (face-foreground 'qaf2-noface) 'error)
(qa-err (face-background 'qaf2-noface) 'error)
;; face-font is nil in batch even when :font is in the spec —
;; the batch terminal realizes no fonts
(qa (face-font 'qaf2-ff) nil)
(qa (face-font 'qaf2-f) nil)
(qa (face-font 'default) nil)

;; == face-all-attributes ==
;; 16 attributes, returned as an alist of (ATTR . VALUE) pairs
(qa (length (face-all-attributes 'qaf2-f)) 16)
;; GNU quirk: with no FRAME arg, batch reports every attribute
;; 'unspecified — realized values only show with an explicit frame
(qa (assq :foreground (face-all-attributes 'qaf2-f))
      '(:foreground . unspecified))
(qa (assq :weight (face-all-attributes 'qaf2-f))
      '(:weight . unspecified))
(qa (car (face-all-attributes 'qaf2-f)) '(:family . unspecified))
(qa (assq :foreground (face-all-attributes 'qaf2-f (selected-frame)))
    '(:foreground . "red"))
(qa (assq :background (face-all-attributes 'qaf2-f (selected-frame)))
    '(:background . "blue"))
(qa (assq :weight (face-all-attributes 'qaf2-f (selected-frame)))
    '(:weight . bold))
;; default's realized fg is the pseudo-string, not 'unspecified
(qa (assq :foreground (face-all-attributes 'default (selected-frame)))
    '(:foreground . "unspecified-fg"))
(qa (assq :foreground (face-all-attributes 'default))
      '(:foreground . unspecified))

;; == internal-lisp-face-p / internal-lisp-face-empty-p ==
;; returns the face vector [face attr...] or nil — not a boolean
(qa (vectorp (internal-lisp-face-p 'default)) t)
(qa (length (internal-lisp-face-p 'default)) 20)
(qa (eq (aref (internal-lisp-face-p 'default) 0) 'face) t)
(qa (vectorp (internal-lisp-face-p 'mode-line)) t)
(qa (vectorp (internal-lisp-face-p 'highlight)) t)
(qa (vectorp (internal-lisp-face-p 'region)) t)
(qa (vectorp (internal-lisp-face-p 'fringe)) t)
(qa (vectorp (internal-lisp-face-p 'qaf2-f)) t)
(qa (internal-lisp-face-p 'qaf2-noface) nil)
(qa (internal-lisp-face-p 'car) nil)
(qa (internal-lisp-face-p "x") nil)
(qa (internal-lisp-face-empty-p 'default) nil)
(qa (internal-lisp-face-empty-p 'qaf2-f) nil)
(qa (make-empty-face 'qaf2-empty) 'qaf2-empty)
(qa (internal-lisp-face-empty-p 'qaf2-empty) t)

;; == face-documentation ==
(qa (face-documentation 'qaf2-f) "qaf2 doc.")
(qa (face-documentation 'region) "Basic face for highlighting the region.")
(qa (string-prefix-p "Face for the mode lines"
                      (face-documentation 'mode-line)) t)
(qa (face-documentation 'qaf2-noface) nil)

;; == face-id / face-name / alias resolution ==
(qa (integerp (face-id 'qaf2-f)) t)
;; an obsolete alias resolves to the same face → same id
(qa (= (face-id 'qaf2-alias) (face-id 'qaf2-f)) t)
(qa (vectorp (facep 'qaf2-alias)) t)
(qa (vectorp (internal-lisp-face-p 'qaf2-alias)) t)
;; built-in face ids are fixed small integers in GNU 31.1
(qa (face-id 'default) 0)
(qa (face-id 'highlight) 12)
(qa (face-id 'region) 13)
(qa (face-id 'mode-line) 25)
(qa-err (face-id 'qaf2-noface) 'error)
(qa-err (face-id nil) 'error)
;; face-name returns a STRING; an alias keeps its own name
(qa (face-name 'qaf2-f) "qaf2-f")
(qa (face-name 'qaf2-alias) "qaf2-alias")
(qa-err (face-name 'car) 'error)
(qa-err (face-name nil) 'error)

;; == face-inverse-video-p ==
(qa (face-inverse-video-p 'qaf2-iv) t)
(qa (face-inverse-video-p 'qaf2-f) nil)
(qa (face-inverse-video-p 'default) nil)

;; == invert-face ==
;; swaps fg and bg in place; returns the face symbol
(qa (invert-face 'qaf2-inv) 'qaf2-inv)
(qa (face-foreground 'qaf2-inv) "white")
(qa (face-background 'qaf2-inv) "black")
;; inverting twice restores the original colors
(qa (progn (invert-face 'qaf2-inv) (face-foreground 'qaf2-inv)) "black")
(qa (face-background 'qaf2-inv) "white")
;; on an empty face the unspecified pair still swaps positions
(qa (invert-face 'qaf2-empty) 'qaf2-empty)
(qa (face-foreground 'qaf2-empty) "unspecified-bg")
(qa (face-background 'qaf2-empty) "unspecified-fg")
(qa-err (invert-face 'qaf2-noface) 'error)

;; == face remapping (buffer-local face-remapping-alist) ==
;; add-relative returns a (FACE . SPECS) cookie
(qa (with-temp-buffer (face-remap-add-relative 'default '(:foreground "orange")))
    '(default :foreground "orange"))
(qa (with-temp-buffer (face-remap-add-relative 'default 'bold 'italic))
    '(default bold italic))
;; a bare face symbol as spec gives a dotted cookie
(qa (with-temp-buffer (face-remap-add-relative 'default 'highlight))
    '(default . highlight))
;; remapping a nonexistent face is silently accepted
(qa (with-temp-buffer (face-remap-add-relative 'qaf2-noface 'bold))
    '(qaf2-noface . bold))
;; alist entries are (FACE SPECS BASE); newest entry is first
(qa (with-temp-buffer
      (face-remap-add-relative 'default '(:foreground "orange"))
      face-remapping-alist)
    '((default (:foreground "orange") default)))
(qa (with-temp-buffer
      (face-remap-add-relative 'default '(:background "yellow"))
      (face-remap-add-relative 'mode-line '(:weight bold))
      face-remapping-alist)
    '((mode-line (:weight bold) mode-line)
      (default (:background "yellow") default)))
;; remapping is buffer-local: a fresh buffer sees an empty alist
(qa (with-temp-buffer
      (face-remap-add-relative 'default '(:foreground "orange"))
      (with-temp-buffer face-remapping-alist))
    nil)
;; remove-relative eats the cookie, returns the removed spec list
(qa (with-temp-buffer
      (let ((ck (face-remap-add-relative 'default '(:slant italic))))
        (list (face-remap-remove-relative ck) face-remapping-alist)))
    '((:slant italic) nil))
;; set-base rewrites the BASE element; reset-base restores the face itself
(qa (with-temp-buffer
      (face-remap-add-relative 'default '(:foreground "red"))
      (face-remap-set-base 'default '(:foreground "blue"))
      (assq 'default face-remapping-alist))
    '(default (:foreground "red") (:foreground "blue")))
(qa (with-temp-buffer
      (face-remap-add-relative 'default '(:foreground "red"))
      (face-remap-set-base 'default '(:foreground "blue"))
      (face-remap-reset-base 'default)
      (assq 'default face-remapping-alist))
    '(default (:foreground "red") default))
(qa (with-temp-buffer (face-remap-set-base 'default '(:foreground "blue")))
    nil)

;; == face-spec-set / face-spec-choose / face-spec-recalc ==
;; face-spec-set creates the face but does NOT store a defface spec
(qa (face-spec-set 'qaf2-ss '((t (:foreground "green")))) nil)
(qa (vectorp (facep 'qaf2-ss)) t)
(qa (face-foreground 'qaf2-ss) "green")
(qa (get 'qaf2-ss 'face-defface-spec) nil)
;; an explicit SPEC-TYPE stores the spec on that property
(qa (face-spec-set 'qaf2-f '((t (:slant normal))) 'face-override-spec) nil)
(qa (get 'qaf2-f 'face-override-spec) '((t (:slant normal))))
(qa (face-attribute 'qaf2-f :slant) 'normal)
(qa (face-spec-recalc 'qaf2-f nil) nil)
;; a nil spec is accepted (resets nothing to display)
(qa (face-spec-set 'qaf2-ss2 nil) nil)

;; face-spec-choose returns the PLIST of the first matching clause
(qa (face-spec-choose '((t :foreground "green")) nil 0)
    '(:foreground "green"))
(qa (face-spec-choose '((((type tty)) :foreground "cyan")
                        (t :foreground "green")) nil 0)
    '(:foreground "cyan"))
;; the batch terminal is a tty: the (type x) clause is skipped
(qa (face-spec-choose '((((type x)) :foreground "cyan")
                        (t :foreground "green")) nil 0)
    '(:foreground "green"))
;; no clause matches → returns the N argument unchanged (odd but true)
(qa (face-spec-choose '((((type x)) :foreground "cyan")) nil 0) 0)
(qa (face-spec-choose '((((type x)) :foreground "cyan")) nil 5) 5)
;; requirements must be list-wrapped; bare symbols are a type error
(qa-err (face-spec-choose '(((type tty) :foreground "cyan")) nil 0)
        'wrong-type-argument)

;; face-spec-set-match-display returns the matched requirement list
(qa (face-spec-set-match-display '((type tty)) (selected-frame)) '(tty))
(qa (face-spec-set-match-display '((type x)) (selected-frame)) nil)
(qa (face-spec-set-match-display t (selected-frame)) t)
(qa (face-spec-set-match-display nil (selected-frame)) t)
(qa (face-spec-set-match-display '((class color)) (selected-frame)) nil)

;; face-spec-reset-face: resets attrs to the defface spec AND stores nils
(qa (defface qaf2-r '((t (:foreground "green"))) "r") 'qaf2-r)
(qa (set-face-attribute 'qaf2-r nil :foreground "pink") nil)
(qa (face-foreground 'qaf2-r) "pink")
(qa (face-spec-reset-face 'qaf2-r) nil)
;; GNU quirk: reset writes nil, not 'unspecified → foreground reads nil
(qa (face-foreground 'qaf2-r) nil)

;; == custom-set-faces ==
;; does NOT create an undefined face
(qa (custom-set-faces '(qaf2-nd ((t (:foreground "magenta"))))) nil)
(qa (facep 'qaf2-nd) nil)
;; on an existing face it applies the spec and records saved-face
(qa (custom-set-faces '(qaf2-r ((t (:foreground "black"))))) nil)
(qa (face-foreground 'qaf2-r) "black")
(qa (get 'qaf2-r 'saved-face) '((t (:foreground "black"))))
(qa (get 'qaf2-r 'customized-face) nil)

;; == face-attr-construct ==
;; plist of the non-unspecified attributes, sorted alphabetically
(qa (face-attr-construct 'qaf2-f)
    '(:background "blue" :foreground "red" :slant normal :weight bold))
(qa (face-attr-construct 'qaf2-iv) '(:inverse-video t))
(qa (make-empty-face 'qaf2-e2) 'qaf2-e2)
(qa (face-attr-construct 'qaf2-e2) nil)

;; == merge-face-attribute ==
;; a specified VALUE always wins over VALUE1
(qa (merge-face-attribute :foreground "red" "blue") "red")
(qa (merge-face-attribute :foreground 'unspecified "blue") "blue")
(qa (merge-face-attribute :underline 'unspecified t) t)
;; relative heights multiply, absolute heights replace
(qa (merge-face-attribute :height 1.5 100) 150)
(qa (merge-face-attribute :height 140 100) 140)
(qa (merge-face-attribute :weight 'bold 'normal) 'bold)
(qa (merge-face-attribute :inherit 'qaf2-f 'default) 'qaf2-f)

;; == face-attribute-relative-p ==
;; only a float :height counts as relative
(qa (face-attribute-relative-p :height 1.2) t)
(qa (face-attribute-relative-p :height 120) nil)
(qa (face-attribute-relative-p :weight 'bold) nil)
(qa (face-attribute-relative-p :foreground "red") nil)
(qa (face-attribute-relative-p :underline t) nil)
(qa (face-attribute-relative-p :inverse-video t) nil)
(qa (face-attribute-relative-p :bogus 1) nil)

;; == frame-face-alist / internal face constructors ==
(qa (consp (frame-face-alist)) t)
(qa (eq (car (assq 'qaf2-f (frame-face-alist))) 'qaf2-f) t)
(qa (vectorp (cdr (assq 'qaf2-f (frame-face-alist)))) t)
(qa (eq (aref (cdr (assq 'qaf2-f (frame-face-alist))) 0) 'face) t)
;; internal-make-lisp-face registers an empty face and returns its vector
(qa (vectorp (internal-make-lisp-face 'qaf2-imf)) t)
(qa (vectorp (internal-lisp-face-p 'qaf2-imf)) t)
;; empty-p rejects faces built by internal-make-lisp-face (not "real")
(qa-err (internal-lisp-face-empty-p 'qaf2-imf) 'error)
;; internal-copy-lisp-face copies frame attrs to a new face
(qa (internal-copy-lisp-face 'qaf2-f 'qaf2-cp (selected-frame) nil) 'qaf2-cp)
(qa (face-foreground 'qaf2-cp) "red")
(qa (face-background 'qaf2-cp) "blue")
(qa (internal-merge-in-global-face 'qaf2-cp (selected-frame)) nil)
(qa (clear-face-cache) nil)

;; == colors: the batch tty knows exactly the 8 ANSI colors ==
(qa (defined-colors)
    '("black" "red" "green" "yellow" "blue" "magenta" "cyan" "white"))
(qa (defined-colors (selected-frame))
    '("black" "red" "green" "yellow" "blue" "magenta" "cyan" "white"))
(qa (color-defined-p "red") t)
(qa (color-defined-p "#f00") t)
(qa (color-defined-p "qaf2-nocolor") nil)
(qa (color-defined-p 5) nil)
;; color-values returns 16-bit RGB triplets
(qa (color-values "red") '(65535 0 0))
(qa (color-values "black") '(0 0 0))
(qa (color-values "#f00") '(65535 0 0))
(qa (color-values "qaf2-nocolor") nil)
;; numbered tty colors ("color-N") are not accepted by name
(qa (color-values "color-1") nil)
(qa (x-color-values "red") '(65535 0 0))
(qa (x-color-values "qaf2-nocolor") nil)

;; == display color capabilities in batch ==
(qa (display-color-p) nil)
(qa (tty-display-color-p) nil)
(qa (tty-display-color-cells) 0)
(qa (display-color-cells) 0)
(qa (display-planes) 3)
(qa (display-supports-face-attributes-p '(:foreground "red")) nil)
(qa (display-supports-face-attributes-p '(:underline t)) nil)

;; == color-distance / color-gray-p ==
(qa (color-distance "red" "red") 0)
;; fixed tty palette → deterministic distance
(qa (color-distance "red" "blue") 327669)
(qa (color-distance "red" '(65535 0 0)) 0)
(qa-err (color-distance "qaf2-nocolor" "red") 'error)
(qa (color-gray-p "gray50") t)
(qa (color-gray-p "white") t)
(qa (color-gray-p "black") t)
(qa (color-gray-p "red") nil)
(qa (color-gray-p "qaf2-nocolor") nil)

;; == absent in GNU 31.1 batch ==
(qa (fboundp 'face-foreground-p) nil)
(qa (fboundp 'face-background-p) nil)
(qa (fboundp 'face-inverse-p) nil)
(qa (fboundp 'face-attrs-match-p) nil)
(qa (fboundp 'tty-defined-color-alist) nil)

(princ (qa--done))
