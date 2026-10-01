;; Modes (define-derived-mode, derived-mode-p, major-mode), function-get/put,
;; package.el surface — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - (derived-mode-p 'prog-mode) → t inside emacs-lisp-mode.
;;   - emacs-lisp-mode parent = prog-mode + lisp-data-mode.
;;   - define-derived-mode creates a mode with major-mode set.
;;   - function-put/function-get sync to symbol plist.
;;   - package-initialize / package-installed-p are fns; package-archives
;; == major-mode / derived-mode-p ==
(with-temp-buffer
  (emacs-lisp-mode)
  ;; derived-mode-p returns the matched MODE symbol (not a list)
  (qa (derived-mode-p 'prog-mode) 'prog-mode)
  (qa (derived-mode-p 'lisp-data-mode) 'lisp-data-mode)
  (qa (derived-mode-p 'text-mode) nil)
  (qa major-mode 'emacs-lisp-mode)
  (qa (eq major-mode 'emacs-lisp-mode) t))

;; define-derived-mode
(define-derived-mode qa--my-mode prog-mode "MyMode" "A test mode.")
(with-temp-buffer
  (qa--my-mode)
  (qa major-mode 'qa--my-mode)
  (qa (derived-mode-p 'prog-mode) 'prog-mode))
;; mode-name string "MyMode" is honored
(with-temp-buffer
  (qa--my-mode)
  (qa mode-name "MyMode"))

;; == function-get / function-put ==
(qa (fboundp 'function-get) t)
(qa (fboundp 'function-put) t)
(function-put 'qa--fsym 'doc 'test-doc)
(qa (function-get 'qa--fsym 'doc) 'test-doc)
(qa (get 'qa--fsym 'doc) 'test-doc)

(qa (boundp 'package-user-dir) t)
(qa (fboundp 'list-packages) t)
(qa (fboundp 'package-initialize) t)
(qa (fboundp 'package-installed-p) t)
(qa (fboundp 'package-install) t)
(qa (fboundp 'package-delete) t)
;; package-archives unbound until package.el loads
(qa (boundp 'package-archives) nil)
;; package-installed-p on a bogus package → nil
(qa (package-installed-p 'no-such-package-xyz-abc) nil)
;; after require, archives is a list with gnu
(require 'package)
(qa (listp package-archives) t)
(qa (car (assoc "gnu" package-archives)) "gnu")

;; == tabulated-list ==
(qa (fboundp 'tabulated-list-mode) t)
(qa (fboundp 'tabulated-list-print) t)
(qa (fboundp 'tabulated-list-init-header) t)
(qa (fboundp 'tabulated-list-sort) t)
(qa (fboundp 'tabulated-list-get-id) t)
(qa (fboundp 'tabulated-list-get-entry) t)
(qa (fboundp 'tabulated-list-set-col) t)
(qa (fboundp 'tabulated-list-put-tag) t)
(qa (boundp 'tabulated-list-format) t)
(qa (boundp 'tabulated-list-entries) t)
(qa (boundp 'tabulated-list-sort-key) t)
(qa (boundp 'tabulated-list-padding) t)
(qa (boundp 'tabulated-list-printer) t)
(qa (boundp 'tabulated-list-groups) t)
(qa (boundp 'tabulated-list-revert-hook) t)
(qa (boundp 'tabulated-list-mode-hook) t)

;; == mode-name / mode-line vars ==
(qa (boundp 'mode-name) t)
(qa (boundp 'mode-line-format) t)
(qa (boundp 'header-line-format) t)
(qa (boundp 'tab-line-format) t)

;; == delay-mode-hooks / run-mode-hooks ==
(qa (fboundp 'delay-mode-hooks) t)
(qa (fboundp 'run-mode-hooks) t)
(qa (fboundp 'run-hooks) t)
(qa (fboundp 'add-hook) t)
(qa (fboundp 'remove-hook) t)
(qa (fboundp 'make-local-hook) nil)

(princ (qa--done))
