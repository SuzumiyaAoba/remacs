;;; -*- lexical-binding: nil; -*-
;; The package metadata library must load past lm-crack-copyright.
(load-file (expand-file-name "../lisp/lisp-mnt.el"
                             (file-name-directory load-file-name)))
(qa (featurep 'lisp-mnt) t)
(qa (fboundp 'lm-summary) t)
(princ (qa--done))
