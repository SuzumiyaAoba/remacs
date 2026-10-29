;;; -*- lexical-binding: t; -*-
;; Remacs extension for #1312; GNU 31.1 keeps the previous indent here.
;; Avoid loading man.el's ANSI-color setup; woman only needs this parent
;; button type for the functions exercised below.
(require 'button)
(define-button-type 'Man-abstract-xref-man-page)
(defvar Man-mode-map (make-sparse-keymap))
(provide 'man)
(require 'woman)

(when qa--remacs
  (qa (with-temp-buffer
        (insert "0\n")
        (goto-char (point-min))
        (let ((woman-prevailing-indent 4))
          (woman2-get-prevailing-indent t)
          woman-prevailing-indent))
      0))

(princ (qa--done)) (terpri)
