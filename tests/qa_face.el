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

(princ (qa--done))
