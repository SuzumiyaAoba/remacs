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
;; "native:string-byte-kind"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((f (audit-native-compile (lambda () "\200\377")))) (list (multibyte-string-p (funcall f)) (string-to-list (funcall f)))) t))
           (error (list 'error err))))
  (terpri))

;; "native:string-byte-kind-baseline"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (multibyte-string-p "\200\377") (string-to-list "\200\377")) t))
           (error (list 'error err))))
  (terpri))
)
  (delete-directory audit-native-dir t))
