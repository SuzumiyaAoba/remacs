;;; qa_lisp_mode_reload.el --- Explicit reload remains equivalent -*- lexical-binding: t; -*-

(load "lisp-mode" nil t)

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

(qa (qa338-status #'lisp-mode)
    '(lisp-mode common-lisp-indent-function t t t lisp-find-tag-default "\\(\\(^\\|[^\\
]\\)\\(\\\\\\\\\\)*\\)\\(;+\\|#|\\) *" "[ 	]*\\(\\s>\\||#\\)" "|#" t 34 39))

(qa (qa338-status #'lisp-data-mode)
    '(lisp-data-mode lisp-indent-function t t t nil ";+ *" nil nil nil 95 39))

(qa (qa338-faces #'lisp-mode t)
    '(font-lock-keyword-face font-lock-function-name-face))

(qa (qa338-indent #'lisp-mode "(defun foo (x)\n(if x\n(+ x 1)\n0))")
    '"(defun foo (x)
  (if x
      (+ x 1)
      0))")

(princ (qa--done))
