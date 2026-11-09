;;; -*- lexical-binding: nil; -*-
;; Loading the batch-file mode must define commands after bat-cmd-help.
(load-file (expand-file-name "../lisp/bat-mode.el"
                             (file-name-directory load-file-name)))
(qa (fboundp 'bat-cmd-help) t)
(qa (fboundp 'bat-run) t)
(princ (qa--done))
