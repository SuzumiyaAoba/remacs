;;; Normalize RFC 822 zones to their numeric offsets (Issue #1849).

(require 'timezone)

;; Evaluate the source function without loading the full Gnus stack.
(let ((source (expand-file-name "lisp/nnrss.el" default-directory)))
  (with-temp-buffer
    (set-syntax-table emacs-lisp-mode-syntax-table)
    (insert-file-contents source)
    (goto-char (point-min))
    (search-forward "(defun nnrss-normalize-date")
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)) t)))

(qa (mapcar (lambda (date)
              (substring (nnrss-normalize-date date) -5))
            '("Thu, 21 Dec 2000 16:01:07 +0200"
              "Wed, 09 Jun 2021 10:18:14 PST"
              "Wed, 09 Jun 2021 10:18:14 EST"))
    '("+0200" "-0800" "-0500"))

(princ (qa--done))
