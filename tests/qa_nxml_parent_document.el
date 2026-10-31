;; A parent document's internal entities remain available in its child.
(require 'nxml-mode)
(require 'rng-nxml)

(qa
 (let* ((parent (make-temp-file "remacs-nxml-parent-"))
        (child (make-temp-file "remacs-nxml-child-"))
        parent-dtd child-state)
   (unwind-protect
       (progn
         (with-temp-file parent
           (insert "<?xml version=\"1.0\"?>\n"
                   "<!DOCTYPE doc [<!ENTITY sample \"hello\">]>\n"
                   "<doc>&sample;</doc>\n"))
         (with-temp-file child
           (insert "<doc>&sample;</doc>\n"))
         (with-current-buffer (find-file-noselect parent)
           (nxml-mode)
           (rng-validate-mode 1)
           (rng-validate-while-idle (current-buffer))
           (setq parent-dtd rng-dtd))
         (with-current-buffer (find-file-noselect child)
           (nxml-mode)
           (rng-validate-mode 1)
           (nxml-parent-document-set parent)
           (setq child-state (list rng-dtd rng-error-count)))
         (list (assoc "sample" parent-dtd)
               (assoc "sample" (car child-state))
               (cadr child-state)))
     (when-let ((buffer (get-file-buffer child)))
       (kill-buffer buffer))
     (when-let ((buffer (get-file-buffer parent)))
       (kill-buffer buffer))
     (delete-file child)
     (delete-file parent)))
 '(("sample" "hello" . "hello")
   ("sample" "hello" . "hello")
   0))

(princ (qa--done))
