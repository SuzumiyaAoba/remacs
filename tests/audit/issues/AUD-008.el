;;; -*- lexical-binding: t; -*-
(defvar audit-native-dir (make-temp-file "remacs-audit-native-" t))
(defvar audit-native-serial 0)
(defvar audit-native-special 1)
(defvar audit-native-side-effect nil)
(defvar audit-native-loaded nil)
(setq user-emacs-directory audit-native-dir)
(setq native-comp-eln-load-path (list audit-native-dir))

(defun audit-native-compile (fn)
  (setq audit-native-serial (1+ audit-native-serial))
  (native-compile fn (expand-file-name (format "case%d.eln" audit-native-serial)
                                      audit-native-dir)))
(unwind-protect
    (progn
;; "native:file-top-level-forms"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((source (expand-file-name "top-level.el" audit-native-dir)) (output (expand-file-name "top-level.eln" audit-native-dir))) (with-temp-file source (insert ";;; -*- lexical-binding: t -*-\n" "(setq audit-native-loaded 42)\n" "(defun audit-native-file-function () 7)\n" "(provide 'audit-native-file-feature)\n")) (native-compile source output) (setq audit-native-loaded nil) (native-elisp-load output) (list audit-native-loaded (featurep 'audit-native-file-feature) (audit-native-file-function))) t))
           (error (list 'error err))))
  (terpri))
)
  (delete-directory audit-native-dir t))
