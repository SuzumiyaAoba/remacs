;;; qa_package_min_dependency.el --- Package dependency version regression -*- lexical-binding:t -*-

(require 'package-activate)

(qa
 (let ((root (make-temp-file "remacs-package-min-dependency-" t)))
   (unwind-protect
       (let* ((dep-dir (expand-file-name "qa-dep-1.0" root))
              (app-dir (expand-file-name "qa-app-1.0" root)))
         (make-directory dep-dir)
         (make-directory app-dir)
         (let* ((dep (package-desc-create
                      :name 'qa-dep :version '(1 0) :dir dep-dir))
                (app (package-desc-create
                      :name 'qa-app :version '(1 0)
                      :reqs '((qa-dep (2 0))) :dir app-dir))
                (package-alist `((qa-app ,app) (qa-dep ,dep)))
                (package-activated-list nil)
                (package-load-list '(all))
                (package--builtin-versions nil)
                (package--builtins nil)
                (package--quickstart-pkgs nil))
           (list (package-activate 'qa-app)
                 (memq 'qa-app package-activated-list)
                 (memq 'qa-dep package-activated-list))))
     (delete-directory root t)))
 '(nil nil nil))

(princ (qa--done))
