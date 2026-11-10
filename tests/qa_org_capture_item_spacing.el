;;; qa_org_capture_item_spacing.el --- Org capture list-spacing regression -*- lexical-binding:t -*-

(require 'org-capture)

(qa
 (let ((org-blank-before-new-entry
        '((heading . t) (plain-list-item . t))))
   (with-temp-buffer
     (org-mode)
     (insert "* Parent\n- old\n")
     (let ((org-capture-plist '(:type item :template "- new")))
       (org-capture-place-item)
       (save-restriction
         (widen)
         (buffer-string)))))
 "* Parent\n- old\n\n- new\n")

(princ (qa--done))
