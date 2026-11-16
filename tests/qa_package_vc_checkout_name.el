;;; qa_package_vc_checkout_name.el --- package-vc path validation regression -*- lexical-binding: t -*-

(require 'cl-lib)
(require 'package-vc)

(qa
 (let* ((root (make-temp-file "package-vc-path-probe-" t))
        (package-user-dir (expand-file-name "elpa/" root))
        (escaped (expand-file-name "escaped" root))
        (marker (expand-file-name "keep" escaped))
        deleted-path
        created-path
        prompted
        result)
   (unwind-protect
       (progn
         (make-directory escaped)
         (with-temp-file marker (insert "keep"))
         (setq result
               (condition-case error
                   (progn
                     (cl-letf (((symbol-function 'package-vc--archives-initialize)
                                #'ignore)
                               ((symbol-function 'package-vc--unpack-1)
                                #'ignore)
                               ((symbol-function 'yes-or-no-p)
                                (lambda (&rest _)
                                  (setq prompted t)
                                  t))
                               ((symbol-function 'package--delete-directory)
                                (lambda (directory)
                                  (setq deleted-path directory)))
                               ((symbol-function 'make-directory)
                                (lambda (directory &optional _parents)
                                  (setq created-path directory)))
                               )
                       (package-vc-install-from-checkout root "../escaped"))
                     'returned)
                 (error (car error))))
         (list result deleted-path created-path prompted
               (file-exists-p marker)))
     (delete-directory root t)))
 '(user-error nil nil nil t))

(qa
 (let* ((root (make-temp-file "package-vc-safe-name-" t))
        (package-user-dir (expand-file-name "elpa/" root))
        created-path)
   (unwind-protect
       (progn
         (cl-letf (((symbol-function 'package-vc--archives-initialize)
                    #'ignore)
                   ((symbol-function 'package-vc--unpack-1) #'ignore)
                   ((symbol-function 'make-directory)
                    (lambda (directory &optional _parents)
                      (setq created-path directory))))
           (package-vc-install-from-checkout root "safe-package"))
         (equal created-path (expand-file-name "safe-package" package-user-dir)))
     (delete-directory root t)))
 t)

(princ (qa--done))
