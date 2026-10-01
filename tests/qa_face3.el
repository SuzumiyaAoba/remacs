;; faces — GNU 31.1 pins (verified in -Q batch): facep/face-list/
;; face-attribute/face-id/make-empty-face/set-face-attribute/invert-face/
;; copy-face/face-all-attributes/face-documentation.
;; NOTE: `qa' evals FORMs dynamically — `let' must be INSIDE each form.
;;
;; GNU truths pinned (batch):
;;   - facep returns the 21-slot attribute vector [face ...unspecified].
;;   - face-id 'default = 0, 'bold = 1.
;;   - face-attribute on 'default: :foreground/:background are the STRINGS
;;     "unspecified-fg"/"unspecified-bg"; :family/:foundry are "default";
;;     :height is 1; :weight/:slant are 'normal; :distant-foreground/:box
;;     are 'unspecified/nil respectively.
;;   - face-foreground on 'bold → nil (its fg is 'unspecified symbol);
;;     face-attribute 'bold :foreground → 'unspecified.
;;   - face-bold-p returns the list of matching styles, not just t.
;;   - face-all-attributes returns a 16-slot alist, all 'unspecified.
;;   - invert-face swaps :foreground/:background, returns the face name.
;;   - copy-face returns the new name; copying onto self is a no-op.
;;   - face-attribute on a non-face signals 'error (not a typed error).

;; == facep returns the attribute vector ==
(qa (vectorp (facep 'default)) t)
(qa (length (facep 'default)) 20)
(qa-same (aref (facep 'default) 0) 'face)
(qa (facep 'no-such-face-qa3) nil)
(qa (facep nil) nil)
(qa (facep 'bold) (facep 'bold))
;; == face-list ordering: 'default is LAST, 'bold just before ==
(qa (car (last (face-list))) 'default)
(qa (memq 'bold (face-list)) '(bold default))
(qa (consp (memq 'italic (face-list))) t)

;; == face-id ==
(qa (face-id 'default) 0)
(qa (face-id 'bold) 1)
(qa (integerp (face-id 'italic)) t)
(qa-err (face-id 'no-such-face-qa3) 'error)

;; == face-attribute on built-in faces ==
(qa (face-attribute 'default :foreground) "unspecified-fg")
(qa (face-attribute 'default :background) "unspecified-bg")
(qa (face-attribute 'default :family) "default")
(qa (face-attribute 'default :foundry) "default")
(qa (face-attribute 'default :height) 1)
(qa-same (face-attribute 'default :weight) 'normal)
(qa-same (face-attribute 'default :slant) 'normal)
(qa (face-attribute 'default :underline) nil)
(qa (face-attribute 'default :overline) nil)
(qa (face-attribute 'default :strike-through) nil)
(qa (face-attribute 'default :extend) nil)
(qa (face-attribute 'default :inherit) nil)
(qa (face-attribute 'default :stipple) nil)
(qa-same (face-attribute 'default :distant-foreground) 'unspecified)
(qa-same (face-attribute 'bold :weight) 'bold)
(qa-same (face-attribute 'italic :slant) 'italic)
(qa (face-attribute 'underline :underline) t)
(qa-same (face-attribute 'bold :foreground) 'unspecified)
(qa-same (face-attribute 'mode-line :box) 'unspecified)
;; non-face signals a generic error
(qa-err (face-attribute 'no-such-face-qa3 :weight) 'error)
;; INHERIT arg 'default → bare value
(qa-same (face-attribute 'default :weight nil 'default) 'normal)

;; == face-foreground / face-background ==
(qa (face-foreground 'default) "unspecified-fg")
(qa (face-background 'default) "unspecified-bg")
;; 'bold has unspecified fg → face-foreground yields nil (symbol case)
(qa (face-foreground 'bold) nil)

;; == style predicates return style lists / booleans ==
(qa (face-bold-p 'bold) '(bold extra-bold ultra-bold))
(qa (face-italic-p 'italic) '(italic oblique))
(qa (face-bold-p 'default) nil)
(qa (face-italic-p 'default) nil)
(qa (face-underline-p 'underline) t)
(qa (face-underline-p 'default) nil)
(qa (face-inverse-video-p 'bold) nil)
(qa (face-font 'default) nil)

;; == face-all-attributes: 16 slots, every value 'unspecified ==
(qa (length (face-all-attributes 'default)) 16)
(qa (assq :foreground (face-all-attributes 'default))
      '(:foreground . unspecified))
(qa (assq :weight (face-all-attributes 'default))
      '(:weight . unspecified))
(qa (cdr (assq :slant (face-all-attributes 'default))) 'unspecified)

;; == make-empty-face / set-face-attribute ==
(qa (let ((f (make-empty-face 'qa3-a)))
      (list (eq f 'qa3-a) (vectorp (facep 'qa3-a))))
    '(t t))
(qa (let ((f (make-empty-face 'qa3-b)))
      (face-attribute f :weight))
    'unspecified)
(qa (let ((f (make-empty-face 'qa3-c)))
      (set-face-attribute f nil :weight 'bold :slant 'italic)
      (list (face-attribute f :weight) (face-attribute f :slant)))
    '(bold italic))
(qa (let ((f (make-empty-face 'qa3-d)))
      (set-face-attribute f nil :foreground "red" :background "blue")
      (list (face-attribute f :foreground)
            (face-attribute f :background)
            (face-foreground f)
            (face-background f)))
    '("red" "blue" "red" "blue"))
(qa (let ((f (make-empty-face 'qa3-e)))
      (set-face-attribute f nil :height 200)
      (face-attribute f :height))
    200)
;; resetting a slot back to 'unspecified
(qa (let ((f (make-empty-face 'qa3-f)))
      (set-face-attribute f nil :height 200)
      (set-face-attribute f nil :height 'unspecified)
      (face-attribute f :height))
    'unspecified)
;; predicate fns reflect a custom face's attributes
(qa (let ((f (make-empty-face 'qa3-g)))
      (set-face-attribute f nil :weight 'bold)
      (face-bold-p f))
    '(bold extra-bold ultra-bold))
;; :inherit shows up verbatim
(qa (let ((f (make-empty-face 'qa3-h)))
      (set-face-attribute f nil :inherit 'bold)
      (face-attribute f :inherit))
    'bold)

;; == invert-face swaps fg/bg, returns the face name ==
(qa (let ((f (make-empty-face 'qa3-i)))
        (set-face-attribute f nil :foreground "red" :background "blue")
        (list (invert-face f)
              (face-attribute f :foreground)
              (face-attribute f :background)))
      '(qa3-i "blue" "red"))

;; == copy-face ==
(qa (let ((f (copy-face 'bold 'qa3-j)))
      (list (eq f 'qa3-j) (face-attribute 'qa3-j :weight)))
    '(t bold))
;; == face-documentation / face-doc-string ==
(qa (face-documentation 'bold) "Basic bold face.")
(qa (face-doc-string 'bold) "Basic bold face.")
(qa (let ((f (make-empty-face 'qa3-k)))
      (put 'qa3-k 'face-documentation "Doc for qa3-k")
      (list (face-documentation 'qa3-k) (face-doc-string 'qa3-k)))
    '("Doc for qa3-k" "Doc for qa3-k"))

;; == face-differs-from-default-p — nil in batch frame ==
(qa (face-differs-from-default-p 'default) nil)
(qa (face-differs-from-default-p 'bold) nil)
(qa (let ((f (make-empty-face 'qa3-l)))
      (face-differs-from-default-p f))
    nil)

;; == face-equal ==
(qa (face-equal 'default 'default) t)
(qa (face-equal 'bold 'italic) nil)

(princ (qa--done))
