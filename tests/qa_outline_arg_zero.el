;; outline-next-visible-heading must not move when ARG is zero.
(require 'outline)

(qa (with-temp-buffer
      (insert "* H\ntext")
      (outline-mode)
      (goto-char (point-min))
      (outline-next-visible-heading 0)
      (point))
    1)

;; outline-previous-visible-heading delegates to the same function.
(qa (with-temp-buffer
      (insert "* H\ntext")
      (outline-mode)
      (goto-char 5)
      (outline-previous-visible-heading 0)
      (point))
    5)

(princ (qa--done))
