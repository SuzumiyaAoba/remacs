;; faces.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - facep returns the attribute vector (non-nil).
;;   - face-id returns an integer.
;;   - face-foreground/-background return 'unspecified-fg/-bg symbols.
;;   - face-attribute returns 'unspecified-fg/-bg as STRINGS (or syms).
;;   - face-differs-from-default-p is nil for all in a batch frame.
;;   - face-bold-p returns the list of matching styles, not just t.

;; == surface ==
(qa (fboundp 'facep) t)
(qa (fboundp 'face-list) t)
(qa (fboundp 'face-attribute) t)
(qa (fboundp 'face-foreground) t)
(qa (fboundp 'face-background) t)
(qa (fboundp 'face-stipple) t)
(qa (fboundp 'face-font) t)
(qa (fboundp 'face-bold-p) t)
(qa (fboundp 'face-italic-p) t)
(qa (fboundp 'face-underline-p) t)
(qa (fboundp 'face-inverse-video-p) t)
(qa (fboundp 'face-all-attributes) t)
(qa (fboundp 'face-equal) t)
(qa (fboundp 'face-differs-from-default-p) t)
(qa (fboundp 'face-documentation) t)
(qa (fboundp 'face-doc-string) t)
(qa (fboundp 'make-face) t)
(qa (fboundp 'make-empty-face) t)
(qa (fboundp 'copy-face) t)
(qa (fboundp 'face-id) t)
(qa (fboundp 'set-face-attribute) t)
(qa (fboundp 'set-face-attribute-from-resource) t)
(qa (fboundp 'invert-face) t)
(qa (fboundp 'face-spec-set) t)
(qa (fboundp 'face-name) t)
(qa (fboundp 'face-default-spec) t)
(qa (fboundp 'face-user-default-spec) t)
(qa (fboundp 'face-set-after-frame-default) t)
;; NOT fns
(qa (fboundp 'internal-facep) nil)
(qa (fboundp 'face-id-history) nil)
(qa (fboundp 'face-default-stipple) nil)

;; == facep — returns the attribute vector, or nil ==
(qa (not (null (facep 'default))) t)
(qa (not (null (facep 'bold))) t)
;; == face-foreground/-background on 'default — return "unspecified-fg/-bg" STRINGS ==
(qa (face-foreground 'default) "unspecified-fg")
(qa (face-background 'default) "unspecified-bg")

;; == face-id returns an int; default = 0 ==
(qa (integerp (face-id 'default)) t)
(qa (face-id 'default) 0)


;; == face-attribute on 'default ==
;; :family → 'default symbol; :fg/:bg → "unspecified-fg" STRINGS
(qa (equal (face-attribute 'default :family) "default") t)
(qa (equal (face-attribute 'default :foreground) "unspecified-fg") t)
(qa (equal (face-attribute 'default :background) "unspecified-bg") t)
(qa (face-attribute 'default :box) nil)
(qa (face-attribute 'default :slant) 'normal)
(qa (face-attribute 'default :weight) 'normal)

;; == face-list — 'default comes last, 'bold just before ==
(let ((fl (face-list)))
  (qa (listp fl) t)
  (qa (memq 'default fl) '(default))
  (qa (memq 'bold fl) '(bold default)))

;; == face-differs-from-default-p — all nil in batch ==
(qa (face-differs-from-default-p 'bold) nil)
(qa (face-differs-from-default-p 'default) nil)
(qa (face-differs-from-default-p 'italic) nil)

;; == face-equal — equal faces only ==
(qa (face-equal 'default 'default) t)
;; different faces differ
(qa (not (face-equal 'bold 'italic)) t)

;; == face-bold-p / -italic-p / -underline-p return matching-style lists ==
(qa (face-bold-p 'bold) '(bold extra-bold ultra-bold))
(qa (face-italic-p 'italic) '(italic oblique))
;; default isn't bold
(qa (face-bold-p 'default) nil)

;; == face-all-attributes — alist; GNU returns 'unspecified for every slot ==
(let ((aa (face-all-attributes 'default)))
  (qa (consp aa) t)
  (qa (cdr (assq :foreground aa)) 'unspecified)
  (qa (cdr (assq :weight aa)) 'unspecified))

;; == make-empty-face — returns the symbol ==
(let ((f (make-empty-face 'qa-test-face)))
  ;; make-empty-face returns the SYMBOL itself
  (qa f 'qa-test-face)
  ;; the new face exists
  (qa (not (null (facep 'qa-test-face))) t))

;; == set-face-attribute on a temp face ==
(let ((f (make-empty-face 'qa-tmp-face)))
  (set-face-attribute f nil :foreground "red")
  (qa (face-attribute f :foreground) "red"))

;; == copy-face creates a copy ==
(let ((f (copy-face 'bold 'qa-copied-face)))
  (qa (not (null (facep 'qa-copied-face))) t)
  ;; it's a different symbol
  (qa (eq f 'bold) nil))

;; == face-documentation returns a doc string ==
(let ((d (face-documentation 'default)))
  (qa (stringp d) t))

;; == face-spec-set — set via set-face-attribute ==
(let ((f (make-empty-face 'qa-spec-face)))
  (set-face-attribute f nil :weight 'bold)
  (qa (face-attribute f :weight) 'bold))

;; == invert-face swaps fg/bg ==
(let ((f (make-empty-face 'qa-inv-face)))
  (set-face-attribute f nil :foreground "red" :background "blue")
  (invert-face f)
  ;; GNU swaps; remacs doesn't in batch
  (qa (face-attribute f :foreground) "blue")
  (qa (face-attribute f :background) "red"))

;; == face-underline-p ==
(qa (face-underline-p 'underline) t)
(qa (face-underline-p 'default) nil)

;; == `reset' is a pseudo-value accepted for every attribute ==
;; GNU xfaces.c check_lface_attrs treats `reset' like `unspecified';
;; elisp-mode.el's defface relies on it (`:foreground reset').
(let ((f (make-empty-face 'qa-reset-face)))
  (qa (condition-case e
          (progn (set-face-attribute f nil :foreground 'reset) 'ok)
        (error (car e)))
      'ok)
  (qa (condition-case e
          (progn (set-face-attribute f nil :underline 'reset) 'ok)
        (error (car e)))
      'ok)
  (qa (condition-case e
          (progn (set-face-attribute f nil :weight 'reset) 'ok)
        (error (car e)))
      'ok))
;; == FRAME=0 means "all frames plus new-frame defaults" ==
;; GNU set-face-attribute rewrites nil→0 internally (faces.el:861);
;; internal-set-lisp-face-attribute must accept fixnum 0 directly.
(let ((f (make-empty-face 'qa-frame0-face)))
  (qa (condition-case e
          (progn (set-face-attribute f 0 :slant 'italic) 'ok)
        (error (car e)))
      'ok)
  (qa (condition-case e
          (progn (internal-set-lisp-face-attribute f :background "red" 0)
                 'ok)
        (error (car e)))
      'ok)
  ;; ...and the write is visible through the spec layer.
  (qa (face-attribute f :slant) 'italic)
  (qa (face-attribute f :background) "red"))

;; Face getter attributes and internal setter creation (#151).
(qa-err-data (face-attribute 'default :bold) 'error '("Invalid face attribute name" :bold))
(qa-err-data (face-attribute 'default :italic) 'error '("Invalid face attribute name" :italic))
(qa-err-data (face-attribute 'default :fixed-pitch) 'error '("Invalid face attribute name" :fixed-pitch))
(qa-err-data (face-attribute 'default :variable-pitch) 'error '("Invalid face attribute name" :variable-pitch))
(qa-err-data (face-attribute 'default :tty-foreground) 'error '("Invalid face attribute name" :tty-foreground))
(qa-err-data (face-attribute 'default :tty-background) 'error '("Invalid face attribute name" :tty-background))
(qa (face-attribute 'fixed-pitch :family) "Monospace")
(qa (face-attribute 'variable-pitch :family) "Sans Serif")
(qa (internal-set-lisp-face-attribute 'qa151-created :weight 'bold) 'qa151-created)
(qa (not (null (facep 'qa151-created))) t)
(qa (face-attribute 'qa151-created :weight) 'bold)
(qa (internal-set-lisp-face-attribute 'qa151-created :weight 'normal) 'qa151-created)
(qa-err-data (internal-set-lisp-face-attribute 'qa151-bad-weight :weight 'bogus)
             'error '("Invalid face weight" bogus))
(qa (not (null (facep 'qa151-bad-weight))) t)
(qa-err-data (internal-set-lisp-face-attribute 'qa151-bad-height :height 'bogus)
             'error '("Face height does not produce a positive integer" bogus))
(qa (not (null (facep 'qa151-bad-height))) t)
(qa-err-data (internal-set-lisp-face-attribute 'qa151-unknown-global :weight 'bold t)
             'error '("Invalid face" qa151-unknown-global))
(qa-err-data (internal-set-lisp-face-attribute 'qa151-unknown-global :weight 'bogus t)
             'error '("Invalid face" qa151-unknown-global))
(qa-err-data (internal-set-lisp-face-attribute 'qa151-unknown-global :weight 'bold 0)
             'error '("Invalid face" qa151-unknown-global))
(qa (progn (internal-set-lisp-face-attribute 'qa151-legacy :bold t)
           (face-attribute 'qa151-legacy :weight)) 'bold)
(qa (progn (internal-set-lisp-face-attribute 'qa151-legacy :bold nil)
           (face-attribute 'qa151-legacy :weight)) 'normal)
(qa (progn (internal-set-lisp-face-attribute 'qa151-legacy :italic t)
           (face-attribute 'qa151-legacy :slant)) 'italic)
(qa (progn (internal-set-lisp-face-attribute 'qa151-legacy :italic nil)
           (face-attribute 'qa151-legacy :slant)) 'normal)
;; Creation precedes attribute validation, after frame validation.
(qa-err-data (internal-set-lisp-face-attribute 'qa151-invalid-frame :bogus 'bold 5)
             'wrong-type-argument '(frame-live-p 5))
(qa (facep 'qa151-invalid-frame) nil)
(qa-err-data (internal-set-lisp-face-attribute 'qa151-invalid-attr :bogus 'bold)
             'error '("Invalid face attribute name" :bogus))
(qa (not (null (facep 'qa151-invalid-attr))) t)
(qa-err-data (internal-set-lisp-face-attribute 'qa151-global-attr :bogus 'bold t)
             'error '("Invalid face" qa151-global-attr))
(qa (facep 'qa151-global-attr) nil)
(qa (progn (internal-set-lisp-face-attribute 'qa151-reset :bold 'reset)
           (face-attribute 'qa151-reset :weight)) 'reset)
(qa (progn (internal-set-lisp-face-attribute 'qa151-unspecified :italic 'unspecified)
           (face-attribute 'qa151-unspecified :slant)) 'italic)
;; Alias targets must be resolved before implicit face creation.
(qa (progn
      (put 'qa151-alias 'face-alias 'qa151-alias-target)
      (internal-set-lisp-face-attribute 'qa151-alias :weight 'bold)) 'qa151-alias-target)
(qa (face-attribute 'qa151-alias-target :weight) 'bold)
(qa (progn
      (put 'qa151-alias2 'face-alias 'qa151-alias)
      (internal-set-lisp-face-attribute 'qa151-alias2 :weight 'normal)) 'qa151-alias-target)
(qa (face-attribute 'qa151-alias :weight) 'normal)
(qa-err-data (progn (put 'qa151-cycle-a 'face-alias 'qa151-cycle-b)
                    (put 'qa151-cycle-b 'face-alias 'qa151-cycle-a)
                    (internal-set-lisp-face-attribute 'qa151-cycle-a :weight 'bold))
             'circular-list '(qa151-cycle-a))
(princ (qa--done))
