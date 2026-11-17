;;; qa_dom_add_child_before_cl_lib.el --- dom loads cl-lib when used standalone -*- lexical-binding: t; -*-

(qa
 (progn
   (when (featurep 'dom)
     (unload-feature 'dom t))
   ;; Keep CL functions available but make require notice the missing feature.
   (setq features (delq 'cl-lib features))
   (require 'dom)
   (list (featurep 'cl-lib)
         (let ((tree (list 'root nil (list 'a nil) (list 'b nil))))
           (dom-add-child-before tree (list 'x nil) (nth 3 tree))
           tree)))
 '(t (root nil (a nil) (x nil) (b nil))))

(princ (qa--done))
