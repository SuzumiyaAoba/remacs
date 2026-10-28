;;; ob-gnuplot.el is fully readable as top-level forms (Issue #1802).

(qa (with-temp-buffer
      (set-syntax-table emacs-lisp-mode-syntax-table)
      (insert-file-contents (expand-file-name "lisp/ob-gnuplot.el"
                                              default-directory))
      (goto-char (point-min))
      (let (provided)
        (while (progn
                 (forward-comment (point-max))
                 (not (eobp)))
          (when (equal (read (current-buffer)) '(provide 'ob-gnuplot))
            (setq provided t)))
        provided))
    t)

;; Stub Org's loaded dependencies to exercise ob-gnuplot's source load itself.
(provide 'org-macs)
(provide 'ob)
(defun org-assert-version () t)
(require 'ob-gnuplot)
(qa (featurep 'ob-gnuplot) t)

(princ (qa--done))
