;;; qa_lisp_mode_init.el --- Complete startup Lisp mode initialization -*- lexical-binding: t; -*-

(defun qa338-status (mode)
  (with-temp-buffer
    (funcall mode)
    (list major-mode lisp-indent-function (and font-lock-defaults t)
          (nth 2 font-lock-defaults) (local-variable-p 'font-lock-defaults)
          find-tag-default-function comment-start-skip comment-end-skip
          font-lock-comment-end-skip imenu-case-fold-search
          (char-syntax 124) (char-syntax 35))))
(defun qa338-faces (mode uppercase)
  (with-temp-buffer
    (insert (if uppercase "(DEFUN FOO (X) X)" "(defun foo (x) x)"))
    (funcall mode)
    (font-lock-mode 1)
    (font-lock-ensure)
    (list (get-text-property 2 'face) (get-text-property 8 'face))))
(defun qa338-indent (mode text)
  (with-temp-buffer
    (insert text) (funcall mode) (setq-local indent-tabs-mode nil)
    (indent-region (point-min) (point-max))
    (buffer-substring-no-properties (point-min) (point-max))))

;; startup locals lisp-mode
(qa (qa338-status #'lisp-mode)
    '(lisp-mode common-lisp-indent-function t t t lisp-find-tag-default "\\(\\(^\\|[^\\
]\\)\\(\\\\\\\\\\)*\\)\\(;+\\|#|\\) *" "[ 	]*\\(\\s>\\||#\\)" "|#" t 34 39))

;; startup locals lisp-data-mode
(qa (qa338-status #'lisp-data-mode)
    '(lisp-data-mode lisp-indent-function t t t nil ";+ *" nil nil nil 95 39))

;; require uses complete startup definitions
(qa (progn (require 'lisp-mode) (with-temp-buffer (lisp-mode) (list lisp-indent-function (and font-lock-defaults t) find-tag-default-function)))
    '(common-lisp-indent-function t lisp-find-tag-default))

;; fontification lisp-mode uppercase=nil
(qa (qa338-faces #'lisp-mode nil)
    '(font-lock-keyword-face font-lock-function-name-face))

;; fontification lisp-mode uppercase=t
(qa (qa338-faces #'lisp-mode t)
    '(font-lock-keyword-face font-lock-function-name-face))

;; fontification lisp-data-mode uppercase=nil
(qa (qa338-faces #'lisp-data-mode nil)
    '(font-lock-keyword-face font-lock-function-name-face))

;; fontification lisp-data-mode uppercase=t
(qa (qa338-faces #'lisp-data-mode t)
    '(font-lock-keyword-face font-lock-function-name-face))

;; fontification emacs-lisp-mode uppercase=nil
(qa (qa338-faces #'emacs-lisp-mode nil)
    '(font-lock-keyword-face font-lock-function-name-face))

;; fontification emacs-lisp-mode uppercase=t
(qa (qa338-faces #'emacs-lisp-mode t)
    '(nil nil))

;; indentation lisp-mode
(qa (qa338-indent #'lisp-mode "(defun foo (x)\n(if x\n(+ x 1)\n0))")
    '"(defun foo (x)
  (if x
      (+ x 1)
      0))")

;; indentation lisp-mode
(qa (qa338-indent #'lisp-mode "(let ((x 1))\n(when x\n(print x)))")
    '"(let ((x 1))
  (when x
    (print x)))")

;; indentation emacs-lisp-mode
(qa (qa338-indent #'emacs-lisp-mode "(defun foo (x)\n(if x\n(+ x 1)\n0))")
    '"(defun foo (x)
  (if x
      (+ x 1)
    0))")

;; indentation emacs-lisp-mode
(qa (qa338-indent #'emacs-lisp-mode "(let ((x 1))\n(when x\n(print x)))")
    '"(let ((x 1))
  (when x
    (print x)))")

;; Common Lisp find-tag helper strips package prefix
(qa (with-temp-buffer (insert "package:foo") (lisp-mode) (goto-char 5) (funcall find-tag-default-function))
    '"foo")

;; Common Lisp mode alias initializes same state
(qa (with-temp-buffer (common-lisp-mode) (list major-mode lisp-indent-function (and font-lock-defaults t)))
    '(lisp-mode common-lisp-indent-function t))

(princ (qa--done))
