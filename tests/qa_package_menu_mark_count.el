;;; qa_package_menu_mark_count.el --- Package Menu prefix argument regression -*- lexical-binding:t -*-

(require 'cl-lib)

(load (expand-file-name
       "../lisp/package.el"
       (file-name-directory load-file-name)))

(defun qa-package-menu-mark-rows (function statuses count)
  "Run FUNCTION over a temporary list with STATUSES and collect its actions."
  (with-temp-buffer
    (insert "one\ntwo\nthree\n")
    (goto-char (point-min))
    (let (actions)
      (cl-letf (((symbol-function 'package--ensure-package-menu-mode)
                 (lambda () nil))
                ((symbol-function 'package-menu-get-status)
                 (lambda () (nth (1- (line-number-at-pos)) statuses)))
                ((symbol-function 'package-menu--overlay-line)
                 (lambda (face)
                   (push (list 'overlay (line-number-at-pos) face) actions)))
                ((symbol-function 'tabulated-list-put-tag)
                 (lambda (tag &optional advance)
                   (push (list 'tag (line-number-at-pos) tag) actions)
                   (when advance (forward-line))))
                ((symbol-function 'package-menu--remove-overlay)
                 (lambda ()
                   (push (list 'remove (line-number-at-pos)) actions))))
        (funcall function count))
      (nreverse actions))))

(qa
 (qa-package-menu-mark-rows
  #'package-menu-mark-delete '("installed" "available" "obsolete") 3)
 '((overlay 1 package-mark-delete-line)
   (tag 1 "D")
   (overlay 3 package-mark-delete-line)
   (tag 3 "D")))

(qa
 (qa-package-menu-mark-rows
  #'package-menu-mark-install '("available" "new" "installed") 3)
 '((overlay 1 package-mark-install-line)
   (tag 1 "I")
   (overlay 2 package-mark-install-line)
   (tag 2 "I")))

(qa
 (qa-package-menu-mark-rows
  #'package-menu-mark-unmark '("installed" "available" "obsolete") 3)
 '((remove 1) (tag 1 " ")
   (remove 2) (tag 2 " ")
   (remove 3) (tag 3 " ")))

(qa (qa-package-menu-mark-rows #'package-menu-mark-unmark '("x") 0) nil)

(princ (qa--done))
