;;; Loading compiler libraries must retain the active native backend. -*- lexical-binding: t; -*-
(defvar qa-ncl-directory (make-temp-file "qa-native-comp-load-" t))
(defvar qa-ncl-hook-result nil)
(defvar qa-ncl-serial 0)
(defun qa-ncl-compile ()
  (setq qa-ncl-serial (1+ qa-ncl-serial))
  (let ((function (native-compile
                   (lambda (value) (+ value 5))
                   (expand-file-name (format "entry-%d.eln" qa-ncl-serial) qa-ncl-directory))))
    (list (native-comp-function-p function) (funcall function 2))))

(unwind-protect
    (progn
      (unless (native-comp-available-p) (error "Actual native compilation is required"))
      (with-eval-after-load 'comp
        (setq qa-ncl-hook-result (qa-ncl-compile)))
      (require 'comp)
      (qa qa-ncl-hook-result '(t 7))
      (qa (qa-ncl-compile) '(t 7))
      (load (locate-library "comp") nil t)
      (qa (qa-ncl-compile) '(t 7))
      (load (locate-library "comp-run") nil t)
      (qa (qa-ncl-compile) '(t 7))
      ;; Unlike load's first-cookie scan, compilation uses safe local variables.
      (let ((file (expand-file-name "lexical.el" qa-ncl-directory))
            (output (expand-file-name "lexical.eln" qa-ncl-directory)))
        (with-temp-file file
          (insert ";;; -*- lexical-binding: false; -*-\n"
                  "(defun qa-ncl-lexical () (let ((qa-ncl-captured 7)) (lambda () qa-ncl-captured)))\n"))
        (native-elisp-load (native-compile file output))
        (qa (native-comp-function-p (symbol-function 'qa-ncl-lexical)) t)
        (qa-err (funcall (qa-ncl-lexical)) 'void-variable))
      (let ((file (expand-file-name "dynamic.el" qa-ncl-directory))
            (output (expand-file-name "dynamic.eln" qa-ncl-directory)))
        (with-temp-file file
          (insert ";;; -*- lexical-binding: nil; lexical-binding: t -*-\n"
                  "(defun qa-ncl-dynamic () (let ((qa-ncl-captured 7)) (lambda () qa-ncl-captured)))\n"))
        (native-elisp-load (native-compile file output))
        (qa (native-comp-function-p (symbol-function 'qa-ncl-dynamic)) t)
        (qa (funcall (qa-ncl-dynamic)) 7))
      ;; A caller-supplied library remains authoritative, even if named comp.
      (let* ((directory (make-temp-file "qa-comp-override-" t))
             (file (expand-file-name "comp.el" directory))
             (saved (symbol-function 'native-compile)))
        (unwind-protect
            (progn
              (with-temp-file file
                (insert ";;; -*- lexical-binding: t; -*-\n"
                        "(defun native-compile (&rest _) 'user-compiler)\n"))
              (load file nil t)
              (qa (native-compile nil) 'user-compiler))
          (fset 'native-compile saved)
          (delete-directory directory t))))
  (delete-directory qa-ncl-directory t))
(princ (qa--done))
