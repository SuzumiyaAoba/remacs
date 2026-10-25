;;; -*- lexical-binding: nil; -*-
;; macOS process-attributes works without spawning external ps (#607).
(when (eq system-type 'darwin)
  (qa
   (let ((attrs (process-attributes (emacs-pid))))
     (list (consp (car attrs))
           (stringp (cdr (assq 'args attrs)))
           (stringp (cdr (assq 'comm attrs)))
           (integerp (cdr (assq 'euid attrs)))
           (integerp (cdr (assq 'egid attrs)))
           (integerp (cdr (assq 'rss attrs)))
           (integerp (cdr (assq 'vsize attrs)))))
   '(t t t t t t t))
  (qa (process-attributes 99999999) nil))
(princ (qa--done))
