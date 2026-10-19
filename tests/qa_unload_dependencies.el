;;; -*- lexical-binding: t; -*-
;; Issue #56: compile-time helpers must not become runtime dependents
;; of cl-lib merely because the bundled source is interpreted.
(qa (progn (require 'cl-lib) (unload-feature 'cl-lib) (featurep 'cl-lib)) nil)
(qa (fboundp 'cl-plusp) nil)
(qa (require 'cl-lib) 'cl-lib)
(qa (cl-plusp 1) t)

;; Source loaded by the user records real dependencies, including an
;; interpreted eval-when-compile, exactly like GNU's source loader.
(defvar qa56-directory)
(defvar qa56-kind)
(defvar qa56-feature)
(defvar qa56-file)
(let ((qa56-directory (make-temp-file "qa56-" t)))
  (unwind-protect
      (progn
        (dolist (qa56-kind '(runtime expansion explicit))
          (let* ((qa56-feature (intern (format "qa56-%s" qa56-kind)))
                 (qa56-file (expand-file-name "simple.el" qa56-directory)))
            (with-temp-file qa56-file
              (insert ";;; -*- lexical-binding: t; -*-\n"
                      (if (eq qa56-kind 'runtime)
                          "(require 'cl-lib)\n"
                        "(eval-when-compile (require 'cl-lib))\n")
                      (format "(provide '%s)\n" qa56-feature)))
            (let ((load-path (cons qa56-directory load-path)))
              (qa (load (if (eq qa56-kind 'explicit) qa56-file "simple") nil t) t))
            (qa-err (unload-feature 'cl-lib) 'error)
            (qa (featurep 'cl-lib) t)
            (qa (unload-feature qa56-feature) nil)))
        (qa (unload-feature 'cl-lib) nil)
        (qa (featurep 'cl-lib) nil)
        (qa (require 'cl-lib) 'cl-lib)
        (qa (cl-plusp 2) t))
    (delete-directory qa56-directory t)))

(princ (qa--done)) (terpri)
