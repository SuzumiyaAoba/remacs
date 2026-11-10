;;; -*- lexical-binding: t; -*-
;; Mode-hook feature adjustments survive a global font-lock level change (#1973).

(require 'treesit)

(qa
 (let ((saved-level (default-toplevel-value 'treesit-font-lock-level)))
   (unwind-protect
       (progn
         (set-default-toplevel-value 'treesit-font-lock-level 3)
         (with-temp-buffer
           (setq-local treesit-font-lock-feature-list '((comment) (keyword)))
           (setq-local treesit-font-lock-settings
                       '((nil nil comment nil nil test-language)
                         (nil nil keyword nil nil test-language)
                         (nil nil emacs-devel nil nil test-language)))
           (cl-letf (((symbol-function 'treesit-available-p) (lambda (&rest _) t))
                     ((symbol-function 'treesit-validate-and-compile-font-lock-rules)
                      (lambda (&rest _) nil))
                     ((symbol-function 'font-lock-flush) (lambda (&rest _) nil)))
             (treesit-font-lock-recompute-features)
             (treesit-font-lock-recompute-features '(emacs-devel) '(comment))
             (let ((before (mapcar #'treesit-font-lock-setting-enable
                                   treesit-font-lock-settings)))
               (treesit--font-lock-level-setter 'treesit-font-lock-level 1)
               (list before
                     (mapcar #'treesit-font-lock-setting-enable
                             treesit-font-lock-settings))))))
     (set-default-toplevel-value 'treesit-font-lock-level saved-level)))
 '((nil t t) (nil nil t)))

(princ (qa--done))
