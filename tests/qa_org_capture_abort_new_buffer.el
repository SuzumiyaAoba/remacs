;;; -*- lexical-binding: nil; -*-
;; Aborting a capture to a newly created target must not save that file.
(provide 'pcomplete)
(provide 'org-pcomplete)
(load-file (expand-file-name "../lisp/org-capture.el"
                             (file-name-directory load-file-name)))
(qa
 (let ((org-capture-plist nil)
       (org-capture-current-plist '(:new-buffer t)))
   (org-capture--target-buffer-should-be-saved-p t))
 nil)
(qa
 (let ((org-capture-plist nil)
       (org-capture-current-plist '(:new-buffer t)))
   (org-capture--target-buffer-should-be-saved-p nil))
 t)
(qa
 (let ((org-capture-plist '(:no-save t))
       (org-capture-current-plist nil))
   (org-capture--target-buffer-should-be-saved-p nil))
 nil)
(princ (qa--done))
