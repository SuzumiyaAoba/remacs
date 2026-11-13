;;; qa_ediff_toolbar.el --- Ediff toolbar behavior -*- lexical-binding:t -*-

(require 'ediff-util)

(qa (featurep 'ediff-tbar) t)

;; Unsupported displays report the reason and leave the preference unchanged.
(qa (let ((ediff-use-toolbar-p nil))
      (condition-case err
          (progn (ediff-toggle-use-toolbar) 'no-error)
        (user-error
         (list (car err)
               ediff-use-toolbar-p
               (and (string-match-p "toolbar" (error-message-string err)) t)))))
    '(user-error nil t))

;; The GUI path installs a buffer-local map and restores the prior map when
;; toggled off.
(qa
 (let ((control (generate-new-buffer " *qa-ediff-toolbar*"))
       (ediff-session-registry nil)
       (ediff-use-toolbar-p nil)
       (original-lines (frame-parameter nil 'tool-bar-lines))
       result)
   (unwind-protect
       (with-current-buffer control
         (setq ediff-control-buffer control)
         (setq-local tool-bar-map (make-sparse-keymap))
         (let ((original tool-bar-map))
           (setq ediff-session-registry (list control))
           (cl-letf (((symbol-function 'ediff-has-toolbar-support-p)
                      (lambda () t))
                     ((symbol-function 'ediff--toolbar-display-enabled-p)
                      (lambda (&optional _frame) t))
                     ((symbol-function 'ediff-recenter)
                      (lambda (&rest _args) nil)))
             (ediff-toggle-use-toolbar)
             (setq result
                   (list ediff-use-toolbar-p
                         (eq tool-bar-map ediff--tool-bar-map)
                         (> (or (frame-parameter nil 'tool-bar-lines) 0) 0)
                         (equal (let ((binding (lookup-key tool-bar-map
                                                          [ediff-next])))
                                  (if (consp binding) (nth 2 binding) binding))
                                'ediff-next-difference)))
             (ediff-toggle-use-toolbar)
             (setq result
                   (append result
                           (list ediff-use-toolbar-p
                                 (eq tool-bar-map original)
                                 (equal (frame-parameter nil 'tool-bar-lines)
                                        original-lines)))))))
     (kill-buffer control))
   result)
 '(t t t t nil t t))

(princ (qa--done))
