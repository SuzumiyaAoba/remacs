;; Customize data API (custom.el core), GNU-compatible.
;; Self-asserting: any mismatch signals `error'.

;; ---------- defcustom / custom-declare-variable ----------
(progn
  (defgroup probe-gA nil "probe group A")
  (defcustom probe-rv 0 "doc"
             :require 'probe-foo-pkg :set-after '(aa bb) :safe 'integerp
             :risky t :package-version '("pkg" . "1.2") :options '(:o1 :o2)
             :tag "MyTag" :link '(custom-group-link g1) :version "9.9"
             :type '(choice integer string) :group 'probe-gA)
  ;; Property metadata, all GNU-verified.
  (cl-assert (equal (get 'probe-rv 'custom-requests) '(probe-foo-pkg)))
  (cl-assert (equal (get 'probe-rv 'custom-dependencies) '(bb aa)))
  (cl-assert (eq (get 'probe-rv 'safe-local-variable) 'integerp))
  (cl-assert (eq (get 'probe-rv 'risky-local-variable) t))
  (cl-assert (equal (get 'probe-rv 'custom-package-version) '("pkg" . "1.2")))
  (cl-assert (equal (get 'probe-rv 'custom-options) '(:o1 :o2)))
  (cl-assert (equal (get 'probe-rv 'custom-tag) "MyTag"))
  (cl-assert (equal (get 'probe-rv 'custom-links) '((custom-group-link g1))))
  (cl-assert (equal (get 'probe-rv 'custom-version) "9.9"))
  (cl-assert (equal (get 'probe-rv 'custom-type) '(choice integer string)))
  (cl-assert (equal (get 'probe-rv 'variable-documentation) "doc"))
  ;; standard-value holds a funcall-thunk form.
  (let ((sv (get 'probe-rv 'standard-value)))
    (cl-assert (and (consp sv) (eq (car (car sv)) 'funcall)))
    (cl-assert (eq (eval (car sv)) 0)))
  ;; Group membership.
  (cl-assert (member '(probe-rv custom-variable) (get 'probe-gA 'custom-group)))
  ;; Value bound to the default.
  (cl-assert (eq probe-rv 0))
  ;; Re-declare: doc/standard-value update, value untouched.
  (defcustom probe-rv 1 "doc2")
  (cl-assert (eq probe-rv 0))
  (cl-assert (equal (get 'probe-rv 'variable-documentation) "doc2"))
  ;; custom-variable-p truthy for declared, nil otherwise.
  (cl-assert (custom-variable-p 'probe-rv))
  (cl-assert (not (custom-variable-p 'probe-undeclared)))
  (cl-assert (not (custom-variable-p 42))))

;; ---------- initializers ----------
(progn
  (custom-initialize-default 'probe-d1 5)
  (cl-assert (boundp 'probe-d1))
  (custom-initialize-set 'probe-s1 6)
  (cl-assert (eq probe-s1 6))
  (custom-initialize-reset 'probe-r1 7)
  (cl-assert (eq probe-r1 7))
  (defun probe--fset (s v) (set s (* v 10)))
  (put 'probe-c1 'custom-set 'probe--fset)
  (custom-initialize-set 'probe-c1 3)
  (cl-assert (eq probe-c1 30))
  (custom-initialize-changed 'probe-ch1 4)
  (cl-assert (eq probe-ch1 4))
  ;; Already-bound: initialize-changed leaves it alone.
  (custom-initialize-changed 'probe-ch1 40)
  (cl-assert (eq probe-ch1 4))
  (custom-initialize-delay 'probe-dv 9)
  (cl-assert (memq 'probe-dv custom-delayed-init-variables))
  (cl-assert (equal (get 'probe-dv 'custom-delayed-init) '(9))))

;; ---------- defgroup ----------
(progn
  (defcustom probe-gv 1 "d" :group 'probe-gX)
  (defgroup probe-gX nil "group doc" :group 'probe-gY)
  (cl-assert (member '(probe-gX custom-group) (get 'probe-gY 'custom-group)))
  (cl-assert (equal (get 'probe-gX 'group-documentation) "group doc"))
  (cl-assert (memq 'probe-gX (custom-group-list 'probe-gY))))

;; ---------- defface / custom-declare-face ----------
(progn
  (defface probe-face '((t (:foreground "red" :weight bold)))
           "face doc" :group 'probe-gA :version "3.0")
  (cl-assert (equal (get 'probe-face 'face-defface-spec)
                    '((t (:foreground "red" :weight bold)))))
  (cl-assert (equal (get 'probe-face 'face-documentation) "face doc"))
  (cl-assert (equal (get 'probe-face 'custom-version) "3.0"))
  (cl-assert (member '(probe-face custom-face) (get 'probe-gA 'custom-group)))
  (cl-assert (custom-facep 'probe-face))
  (cl-assert (facep 'probe-face))
  (cl-assert (not (custom-facep 'probe-noface)))
  ;; face-spec-set creates the face and records override-spec.
  (face-spec-set 'probe-fsface '((t (:foreground "cyan"))))
  (cl-assert (facep 'probe-fsface))
  (cl-assert (equal (get 'probe-fsface 'face-override-spec)
                    '((t (:foreground "cyan")))))
  (cl-assert (equal (face-attribute 'probe-fsface :foreground) "cyan")))

;; ---------- themes ----------
(progn
  (deftheme probe-th "theme doc")
  (cl-assert (eq (get 'probe-th 'theme-feature) 'probe-th-theme))
  (cl-assert (equal (get 'probe-th 'theme-documentation) "theme doc"))
  (cl-assert (custom-theme-p 'probe-th))
  (cl-assert (not (custom-theme-p 'probe-noth)))
  (cl-assert (equal (condition-case e (progn (custom-check-theme 'probe-noth)
                                            'no-err)
                     (error (car e)))
                    'error))
  ;; Unknown-theme error.
  (cl-assert (equal (cdr (condition-case e
                             (progn (custom-check-theme 'probe-noth) 'ok)
                           (error e)))
                    '("Unknown theme ‘probe-noth’"))))

(progn
  (defcustom probe-tv 10 "d" :group 'probe-gA)
  (deftheme probe-t1) (deftheme probe-t2)
  (custom-theme-set-variables 'probe-t1 '(probe-tv 1))
  (custom-theme-set-variables 'probe-t2 '(probe-tv 2))
  (cl-assert (equal (get 'probe-t1 'theme-settings)
                    '((theme-value probe-tv probe-t1 1))))
  (enable-theme 'probe-t1)
  (enable-theme 'probe-t2)
  ;; Most-recently-enabled theme wins; enabled list is newest-first.
  (cl-assert (eq probe-tv 2))
  (cl-assert (equal custom-enabled-themes '(probe-t2 probe-t1)))
  (cl-assert (equal (get 'probe-tv 'theme-value)
                    '((probe-t2 2) (probe-t1 1))))
  ;; Disable restores the previous theme's value.
  (disable-theme 'probe-t2)
  (cl-assert (eq probe-tv 1))
  (cl-assert (equal custom-enabled-themes '(probe-t1)))
  ;; Disabling the last theme restores the standard value.
  (disable-theme 'probe-t1)
  (cl-assert (eq probe-tv 10))
  (cl-assert (null custom-enabled-themes))
  ;; 'changed' entry records a value that differs from standard.
  (defun probe--mul (s v) (set s (* v 100)))
  (defcustom probe-mv 1 "d" :set 'probe--mul)
  (deftheme probe-t3)
  (custom-theme-set-variables 'probe-t3 '(probe-mv 2))
  (enable-theme 'probe-t3)
  (cl-assert (eq probe-mv 200))
  (cl-assert (equal (get 'probe-mv 'theme-value)
                    '((probe-t3 2) (changed 100))))
  (disable-theme 'probe-t3)
  ;; The recorded `changed' value is re-applied through :set.
  (cl-assert (eq probe-mv 10000))
  (cl-assert (equal (get 'probe-mv 'theme-value) '((changed 100)))))

;; custom-push-theme: 'set pushes unconditionally; user theme pushes
;; (user . VAL) onto the symbol's own property.
(progn
  (custom-push-theme 'theme-face 'probe-f1 'probe-th 'set 'specA)
  (custom-push-theme 'theme-face 'probe-f1 'probe-th 'set 'specB)
  (cl-assert (equal (get 'probe-th 'theme-settings)
                    '((theme-face probe-f1 probe-th specB)
                      (theme-face probe-f1 probe-th specA))))
  (custom-push-theme 'theme-value 'probe-uv 'user 'set 7)
  (cl-assert (equal (get 'probe-uv 'theme-value) '((user 7)))))

;; ---------- custom-set-variables / custom-set-faces ----------
(progn
  (defcustom probe-sv 0 "d" :set (lambda (s v) (set s v)))
  (custom-set-variables '(probe-sv 3 t (probe-req-x) "cmt") '(probe-sv2 4))
  (cl-assert (eq probe-sv 3))
  (cl-assert (equal (get 'probe-sv 'saved-value) '(3)))
  (cl-assert (equal (get 'probe-sv 'custom-requests) '(probe-req-x)))
  (cl-assert (equal (get 'probe-sv 'saved-variable-comment) "cmt"))
  ;; Undeclared var: recorded but not applied (stays unbound).
  (cl-assert (not (boundp 'probe-sv2)))
  (cl-assert (equal (get 'probe-sv2 'saved-value) '(4)))
  (cl-assert (equal (get 'probe-sv2 'theme-value) '((user 4)))))

(progn
  (defface probe-pf '((t (:foreground "blue"))) "d")
  (custom-set-faces '(probe-pf ((t (:foreground "green")))))
  (cl-assert (null (get 'probe-pf 'customized-face)))
  (cl-assert (equal (get 'probe-pf 'saved-face) '((t (:foreground "green")))))
  (cl-assert (equal (face-attribute 'probe-pf :foreground) "green")))

;; ---------- customize-set-* ----------
(progn
  (defcustom probe-cv 1 "d")
  (customize-set-variable 'probe-cv 5 "cmt")
  (cl-assert (eq probe-cv 5))
  (cl-assert (equal (get 'probe-cv 'customized-value) '(5)))
  (cl-assert (equal (get 'probe-cv 'customized-variable-comment) "cmt"))
  (cl-assert (null (get 'probe-cv 'saved-value)))
  (customize-save-variable 'probe-cv 8 "sc")
  (cl-assert (eq probe-cv 8))
  (cl-assert (equal (get 'probe-cv 'saved-value) '(8)))
  (cl-assert (equal (get 'probe-cv 'saved-variable-comment) "sc"))
  ;; custom-set-minor-mode calls the function with 1/-1.
  (defun probe-mm (&optional arg) (setq probe-mm-arg arg))
  (custom-set-minor-mode 'probe-mm t)
  (cl-assert (eq probe-mm-arg 1))
  (custom-set-minor-mode 'probe-mm nil)
  (cl-assert (eq probe-mm-arg -1)))

;; ---------- misc ----------
(progn
  (cl-assert (equal (custom-quote '(a (custom-quote (+ 1 2)) c)) '(a 3 c)))
  (custom-note-var-changed 'probe-cv)
  (cl-assert (equal (get 'probe-cv 'customized-value) '(8)))
  (cl-assert (eq (custom-variable-state 'probe-cv) 'saved))
  (cl-assert (eq (custom-face-state 'probe-pf) 'saved))
  (cl-assert (eq (custom-make-theme-feature 'abc) 'abc-theme))
  (provide-theme 'probe-th)
  (cl-assert (featurep 'probe-th-theme)))
