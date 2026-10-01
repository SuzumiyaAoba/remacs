;; custom.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - defgroup + defcustom register a variable with :custom-type.
;;   - custom-variable-p returns the standard-value list (non-nil).
;;   - :group registers on the PARENT's custom-group prop (reversed order).
;;   - custom-initialize-default sets only if SYMBOL is unbound.
;;   - customize-set-value fires :set handler.
;;   - custom-set-variables applies a batch.
;;   - deftheme stores theme-documentation.
;;   - declare-function returns nil.

;; == surface ==
(qa (fboundp 'defgroup) t)
(qa (fboundp 'defcustom) t)
(qa (fboundp 'customize-set-variable) t)
(qa (fboundp 'customize-set-value) t)
(qa (fboundp 'custom-set-variables) t)
(qa (fboundp 'custom-set-faces) t)
(qa (fboundp 'customize-save-variable) t)
(qa (fboundp 'custom-variable-p) t)
(qa (fboundp 'custom-load-symbol) t)
(qa (fboundp 'custom-autoload) t)
(qa (fboundp 'custom-reevaluate-setting) t)
(qa (fboundp 'custom-initialize-default) t)
(qa (fboundp 'custom-initialize-reset) t)
(qa (fboundp 'custom-push-theme) t)
(qa (fboundp 'custom-variable-theme-value) t)
(qa (fboundp 'deftheme) t)
(qa (fboundp 'declare-function) t)
;; NOT fns in batch
(qa (fboundp 'custom-variable-documentation) nil)
(qa (fboundp 'custom-variable-default-form) nil)
(qa (fboundp 'custom-variable-value) nil)
(qa (fboundp 'custom-variable-list) nil)
(qa (fboundp 'custom-group-value) nil)
(qa (fboundp 'custom-default-heading) nil)

;; == defgroup + defcustom ==
(defgroup mygroup nil "test group" :group 'emacs)
(defcustom myvar 42 "test var" :type 'integer :group 'mygroup)

;; the defcustom binds myvar to 42
(qa (symbol-value 'myvar) 42)
;; custom-variable-p returns the standard-value list (non-nil)
(qa (consp (custom-variable-p 'myvar)) t)
;; custom-type recorded
(qa (get 'myvar 'custom-type) 'integer)
;; standard-value is a list with one deferred eval form
(let ((sv (get 'myvar 'standard-value)))
  (qa (listp sv) t)
  (qa (consp (car sv)) t))
;; :group stores on the PARENT, not the variable itself
(qa (get 'myvar 'custom-group) nil)
;; the GROUP's custom-group lists registered members
(let ((members (get 'mygroup 'custom-group)))
  ;; myvar is the newest entry
  (qa (car members) '(myvar custom-variable)))

;; == customize-set-value ==
(defcustom myvar2 0 "v2" :type 'integer :group 'mygroup)
(customize-set-value 'myvar2 99)
(qa (symbol-value 'myvar2) 99)

;; == customize-set-variable ==
(defcustom myvar3 0 "v3" :type 'integer :group 'mygroup)
(customize-set-variable 'myvar3 7)
(qa (symbol-value 'myvar3) 7)

;; == custom-set-variables applies a batch ==
(defcustom myvar4 0 "v4" :type 'integer :group 'mygroup)
(defcustom myvar5 0 "v5" :type 'integer :group 'mygroup)
(custom-set-variables '(myvar4 10) '(myvar5 20))
(qa (symbol-value 'myvar4) 10)
(qa (symbol-value 'myvar5) 20)

;; == defcustom :type recorded ==
(defcustom myvar6 nil "v6" :type 'boolean :group 'mygroup)
(qa (get 'myvar6 'custom-type) 'boolean)

;; == defcustom with :initialize ==
(defcustom myvar7 nil "v7" :type 'string :group 'mygroup
  :initialize 'custom-initialize-default)
;; custom-variable-p returns non-nil (standard-value list)
(qa (consp (custom-variable-p 'myvar7)) t)

;; == defgroup hierarchy — prepends (NAME custom-group) to parent's members ==
(defgroup subgroup nil "sub" :group 'mygroup)
;; == defgroup hierarchy — members APPEND to the end ==
(defgroup subgroup nil "sub" :group 'mygroup)
(let ((members (get 'mygroup 'custom-group)))
  ;; subgroup appended — last member is (subgroup custom-group)
  (qa (car (last members)) '(subgroup custom-group)))

;; == custom-initialize-default skips bound vars ==
(defcustom myvar8 11 "v8" :type 'integer :group 'mygroup)
(setq myvar8 999)
(custom-initialize-default 'myvar8 (get 'myvar8 'standard-value))
(qa (symbol-value 'myvar8) 999)
;; == :set handler — GNU fires it only when a var is "saved" ==
(defvar captured nil)
(defcustom myvar9 0 "v9"
  :type 'integer
  :group 'mygroup
  :set (lambda (sym val) (setq captured (list sym val))))
;; defcustom's :initialize fires :set once → (myvar9 0)
(qa captured '(myvar9 0))
;; custom-set-variables doesn't re-fire :set for unbound vars in 31.1:
;; the defcustom's :set lambda only ran once at init time; since it
;; doesn't bind myvar9, `default-boundp' is nil and the value is just
;; recorded, not applied.
(custom-set-variables '(myvar9 7))
(qa captured '(myvar9 0))

;; == deftheme + declare-function ==
(deftheme mytheme "test theme")
(qa (get 'mytheme 'theme-documentation) "test theme")
(qa (declare-function foo "bar" (x y)) nil)

;; == custom-autoload ==
(defcustom myvar11 0 "v11" :type 'integer :group 'mygroup)
(custom-autoload 'myvar11 'mygroup)

;; == defgroup with nil supergroup ==
(defgroup top nil "top" :group nil)
(qa (get 'top 'custom-group) nil)

;; == boundp / documentation ==
(defcustom myvar10 nil "v10" :type 'sexp :group 'mygroup)
(qa (boundp 'myvar10) t)
(qa (documentation-property 'myvar10 'variable-documentation) "v10")

(princ (qa--done))
