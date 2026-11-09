;;; -*- lexical-binding: nil; -*-
;; A candidate's display string must count when laying out columns.
(load-file (expand-file-name "../lisp/minibuffer.el"
                             (file-name-directory load-file-name)))
(qa
 (with-temp-buffer
   (completion--insert-horizontal
    (list (propertize "x" 'display "123456789") "y")
    nil 1 10 5 2 10)
   (not (null (string-match "\ny" (buffer-string)))))
 t)
(princ (qa--done))
