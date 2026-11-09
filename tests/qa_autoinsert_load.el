;;; -*- lexical-binding: nil; -*-
;; Loading autoinsert and adding an AFTER action must retain both actions.
(load-file (expand-file-name "../lisp/autoinsert.el"
                             (file-name-directory load-file-name)))
(qa
 (let ((auto-insert-alist nil))
   (define-auto-insert 'qa-autoinsert-first "first")
   (define-auto-insert 'qa-autoinsert-first "second" t)
   (cdr (assq 'qa-autoinsert-first auto-insert-alist)))
 ["first" "second"])
(princ (qa--done))
