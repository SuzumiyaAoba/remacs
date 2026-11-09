;;; qa_vc_arch_unregistered.el --- VC-Arch registration-state regression -*- lexical-binding:t -*-

(require 'vc-arch)

(qa
 (let ((root (make-temp-file "remacs-vc-arch-" t)))
   (unwind-protect
       (let* ((default-directory (file-name-as-directory root))
              (admin (expand-file-name "{arch}" root))
              (tagging-method (expand-file-name "=tagging-method" admin))
              (file (expand-file-name "scratch.el" root)))
         (make-directory admin)
         (with-temp-file tagging-method
           (insert "names\n"))
         (with-temp-file file
           (insert "(provide 'scratch)\n"))
         (list (vc-arch-file-source-p file)
               (vc-arch-registered file)
               (vc-arch-state file)))
     (let ((buffer (get-file-buffer
                    (expand-file-name "{arch}/=tagging-method" root))))
       (when buffer (kill-buffer buffer)))
     (delete-directory root t)))
 '(maybe nil unregistered))

(princ (qa--done))
