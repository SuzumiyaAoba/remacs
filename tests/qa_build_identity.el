;;; -*- lexical-binding: nil; -*-
;; Issue #43: stable build metadata and GNU version.el behavior.
(qa emacs-build-time nil)
(qa emacs-build-system nil)
(qa (fboundp 'emacs-build-time) nil)
(qa (stringp system-configuration) t)
(qa (string-prefix-p "GNU Emacs 31.1 (build " (emacs-version)) t)
(qa (string-match-p "\n of " (emacs-version)) nil)
(qa (and (string-match-p (regexp-quote system-configuration) (emacs-version)) t) t)
(qa (version) (emacs-version))
(qa (interactive-form 'emacs-version) '(interactive "P"))
(qa (func-arity #'emacs-version) '(0 . 1))
(qa (with-temp-buffer
      (let ((report (emacs-version)))
        (list (emacs-version t) (equal (buffer-string) report)))) '(nil t))
(qa (let ((system-configuration "qa43-target"))
      (and (string-match-p "qa43-target" (emacs-version)) t)) t)
(qa (let ((emacs-build-number 7))
      (and (string-match-p "(build 7," (emacs-version)) t)) t)
;; Clearing startup details must not remove version.el's documented
;; formatting when callers explicitly supply a build time.
(qa (let ((process-environment (cons "TZ=UTC0" process-environment))
          (emacs-build-time '(0 0 0 0)))
      (string-suffix-p "\n of 1970-01-01" (emacs-version))) t)
(qa (let ((emacs-build-time nil))
      (string-match-p "\n of " (emacs-version))) nil)

(princ (qa--done)) (terpri)
