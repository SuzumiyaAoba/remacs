;;; -*- lexical-binding: t; -*-
;; GNU 31.1 dump inventory and observable provenance. The port may also
;; retain records for its own live bootstrap helpers.
(defun qa122-history (library)
  (catch 'found
    (dolist (entry load-history)
      (when (and (stringp (car entry))
                 (member (file-name-nondirectory (car entry))
                         (list (concat library ".el") (concat library ".elc"))))
        (throw 'found entry)))))
(defun qa122-source (symbol type)
  (let ((file (symbol-file symbol type)))
    (and file (file-readable-p file) (file-name-base file))))
(defconst qa122-dumped-libraries
 '("subdirs" "leim-list" "rmc" "iso-transl" "tooltip" "cus-start" "cconv"
  "eldoc" "shorthands" "paren" "electric" "uniquify" "ediff-hook" "vc-hooks"
  "float-sup" "elisp-mode" "mwheel" "ns-win" "ucs-normalize" "mule-util" "common-win"
  "tool-bar" "dnd" "fontset" "image" "regexp-opt" "fringe" "uni-special-titlecase"
  "uni-special-uppercase" "uni-titlecase" "buff-menu" "tabulated-list" "replace" "newcomment" "fill"
  "text-mode" "lisp-mode" "prog-mode" "paragraphs" "register" "page" "lisp"
  "tab-bar" "menu-bar" "rfn-eshadow" "isearch" "easymenu" "timer" "select"
  "scroll-bar" "mouse" "jit-lock" "font-lock" "syntax" "font-core" "tty-colors"
  "startup" "frame" "minibuffer" "nadvice" "seq" "simple" "cl-generic"
  "indent" "indonesian" "philippine" "cham" "burmese" "khmer" "georgian"
  "utf-8-lang" "misc-lang" "vietnamese" "tibetan" "thai" "tai-viet" "lao"
  "korean" "japanese" "eucjp-ms" "cp51932" "hebrew" "greek" "romanian"
  "slovak" "czech" "european" "ethiopic" "english" "sinhala" "indian"
  "cyrillic" "chinese" "uni-special-lowercase" "composite" "characters" "emoji-zwj" "charscript"
  "uni-lowercase" "uni-uppercase" "uni-category" "uni-brackets" "uni-mirrored" "uni-bidi" "charprop"
  "case-table" "mule-cmds" "epa-hook" "jka-cmpr-hook" "help" "abbrev" "obarray"
  "oclosure" "cl-preloaded" "button" "loaddefs" "theme-loaddefs" "faces" "cus-face"
  "macroexp" "files" "window" "bindings" "format" "env" "mule-conf"
  "mule" "map-ynp" "custom" "widget" "version" "keymap" "subr"
  "backquote" "byte-run" "debug-early"))
(qa (let ((count 0))
      (dolist (library qa122-dumped-libraries count)
        (when (qa122-history library) (setq count (1+ count))))) 136)
(qa (catch 'bad
      (dolist (entry load-history)
        (let ((file (car entry)))
          (when (and (stringp file)
                     (not (and (file-name-absolute-p file) (file-readable-p file))))
            (throw 'bad file))))) nil)
(qa (seq-some (lambda (entry) (and (stringp (car entry))
                                  (string-prefix-p "builtin:" (car entry)))) load-history) nil)
(qa (qa122-history "pcase") nil)
(qa (qa122-history "gv") nil)
(qa (featurep 'subr-x) nil)
(qa (assq 'provide (cdr (qa122-history "subr-x"))) nil)
(qa (qa122-source 'declare-function 'defun) "subr")
(qa (qa122-source 'cl-defgeneric 'defun) "cl-generic")
(qa (qa122-source 'cl-defmethod 'defun) "cl-generic")
(qa (qa122-source 'macroexpand-all 'defun) "macroexp")
(qa (qa122-source 'forward-paragraph 'defun) "paragraphs")
(qa (qa122-source 'gui-get-selection 'defun) "select")
(qa (qa122-source 'seq-reduce 'defun) "seq")
(qa (qa122-source 'paragraph-start 'defvar) "paragraphs")
(qa (qa122-source 'select-enable-clipboard 'defvar) "select")
(qa (qa122-source 'font-lock-mode 'defvar) "font-core")
(qa (qa122-source 'font-lock-warning-face 'defface) "font-lock")
(qa (qa122-source 'region 'defface) "faces")
(qa (symbol-file 'format-mode-line 'defun) nil)
(qa (symbol-file 'copy-marker 'defun) nil)
(qa (symbol-file 'load-history 'defvar) nil)
(qa (symbol-file 'features 'defvar) nil)

(require 'loadhist)
(qa (file-readable-p (feature-file 'macroexp)) t)
(qa (featurep 'paragraphs) nil)
(qa (file-readable-p (feature-file 'select)) t)
(qa (file-name-base (feature-file 'cl-generic)) "cl-generic")
(qa (file-provides "macroexp") '(macroexp))
(qa (file-provides (symbol-file 'forward-paragraph 'defun)) nil)
(qa (file-requires "tooltip") '(syntax))
(qa (sort (mapcar #'file-name-base (file-dependents (feature-file 'syntax))) #'string-lessp)
    '("font-lock" "tooltip"))

;; Re-loading and unloading a real runtime file must keep normal history
;; semantics after restoring the boot manifest.
(defvar qa122-directory (make-temp-file "remacs-history-" t))
(defvar qa122-file (expand-file-name "qa122-module.el" qa122-directory))
(defvar qa122-saved-current nil)
(unwind-protect
    (progn
      (write-region ";;; -*- lexical-binding: t; -*-\n(setq qa122-saved-current current-load-list)\n(defun qa122-module-function () 122)\n(defvar qa122-module-variable 7)\n(provide 'qa122-module)\n"
                    nil qa122-file nil 'quiet)
      (load qa122-file nil t)
      (qa (qa122-module-function) 122)
      (qa (equal (symbol-file 'qa122-module-function 'defun) qa122-file) t)
      (qa (equal (symbol-file 'qa122-module-variable 'defvar) qa122-file) t)
      (qa (equal (feature-file 'qa122-module) qa122-file) t)
      (qa (file-provides qa122-file) '(qa122-module))
      (load qa122-file nil t)
      (qa (eq qa122-saved-current (assoc qa122-file load-history)) t)
      (qa (let ((count 0))
            (dolist (entry load-history count)
              (when (equal (car entry) qa122-file) (setq count (1+ count))))) 1)
      (qa (unload-feature 'qa122-module t) nil)
      (qa (featurep 'qa122-module) nil)
      (qa (fboundp 'qa122-module-function) t)
      (qa (qa122-module-function) 122)
      (qa (boundp 'qa122-module-variable) nil)
      (qa (symbol-file 'qa122-module-function 'defun) nil))
  (delete-directory qa122-directory t))

(qa (let ((name (make-symbol "qa122-history")) (current-load-list nil))
      (fset name 'car) (defalias name 'cdr)
      (list (get name 'function-history) (symbol-function name))) '((nil car) cdr))
(qa (let ((name (make-symbol "qa122-history")) (current-load-list (list "one.el")))
      (fset name 'car) (defalias name 'cdr) (defalias name 'cons)
      (get name 'function-history)) '("one.el" car))
(qa (let ((name (make-symbol "qa122-history")) (current-load-list (list "one.el")))
      (fset name 'car) (defalias name 'cdr)
      (setq current-load-list (list "two.el")) (defalias name 'cons)
      (let ((past (get name 'function-history)))
        (setq current-load-list (list "one.el")) (defalias name 'list)
        (list (get name 'function-history) (eq past (cddr (get name 'function-history))))))
    '(("one.el" cons "two.el" car) t))
(qa (let* ((name (make-symbol "qa122-history"))
           (autoload '(autoload "qa122-autoload" nil nil))
           (current-load-list (list "auto.el")))
      (fset name autoload) (defalias name 'car)
      (eq autoload (cadr (get name 'function-history)))) t)
(qa (let ((name (make-symbol "qa122-history")))
      (fset name 'car) (fset name 'cdr) (get name 'function-history)) nil)

;; Buffer evaluation must establish its own provenance, including a nil
;; filename, instead of attaching definitions to the surrounding load.
(qa (with-temp-buffer
      (setq-local lexical-binding t)
      (insert "(defun qa122-anonymous-buffer () 1)")
      (eval-buffer)
      (symbol-file 'qa122-anonymous-buffer 'defun)) nil)
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qa122-buffer.el")
      (setq-local lexical-binding t)
      (insert "(defun qa122-named-buffer () 2)")
      (eval-buffer)
      (symbol-file 'qa122-named-buffer 'defun)) "/tmp/qa122-buffer.el")
(qa (with-temp-buffer
      (setq-local lexical-binding t)
      (insert "(defun qa122-explicit-buffer () 3)")
      (eval-buffer nil nil "qa122-explicit.el")
      (symbol-file 'qa122-explicit-buffer 'defun)) "qa122-explicit.el")
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qa122-partial.el")
      (setq-local lexical-binding t)
      (insert "(defun qa122-before-region () 1)") (eval-buffer)
      (erase-buffer)
      (insert "\n(defun qa122-first-region () 2)\n(defun qa122-second-region () 3)")
      (narrow-to-region 2 (point-max)) (eval-buffer)
      (let ((items (cdr (assoc "/tmp/qa122-partial.el" load-history))))
        (list (not (null (member '(defun . qa122-before-region) items)))
              (not (null (member '(defun . qa122-first-region) items)))
              (not (null (member '(defun . qa122-second-region) items)))))) '(t t nil))
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qa122-region.el")
      (setq-local lexical-binding t)
      (insert "(defun qa122-region-buffer () 4)")
      (eval-region (point-min) (point-max))
      (symbol-file 'qa122-region-buffer 'defun)) "/tmp/qa122-region.el")

;; The location must actually find the imported definition in source.
(qa (let* ((vc-handled-backends nil)
           (location (find-function-noselect 'cl-defgeneric))
           (buffer (car location)))
      (and (bufferp buffer) (integer-or-marker-p (cdr location))
           (file-readable-p (buffer-file-name buffer)))) t)
(require 'help-fns)
(qa (with-temp-buffer
      (let ((standard-output (current-buffer)))
        (help-fns-function-description-header 'cl-defgeneric))
      (and (string-match-p "cl-generic\\.el" (buffer-string)) t)) t)
(qa (unload-feature 'select t) nil)
(qa (fboundp 'gui-get-selection) nil)
(qa (symbol-file 'gui-get-selection 'defun) nil)

(princ (qa--done))
