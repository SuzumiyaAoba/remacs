;;; audit_native.el --- native compiler semantic probes -*- lexical-binding: t; -*-
;; All generated files go into a temporary directory, removed on completion.
;; Use REMACS_AUDIT_ONLY to run one case in its own process.

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

(defun audit-native-run (id form)
  (unless (and (getenv "REMACS_AUDIT_ONLY")
               (not (equal id (getenv "REMACS_AUDIT_ONLY"))))
    (let ((print-circle t) (print-length 50) (print-level 20)
          (print-escape-newlines t) (print-escape-control-characters t))
      (when (getenv "REMACS_AUDIT_FORMS")
        (princ "FORM|") (prin1 id) (princ "|") (prin1 form) (terpri))
      (princ "AUDIT|") (prin1 id) (princ "|")
      (prin1 (condition-case e (list 'ok (eval form t)) (error (list 'error e))))
      (terpri))))

(unwind-protect
    (progn
      (audit-native-run "native:basic"
       '(funcall (audit-native-compile (lambda (x) (+ x 1))) 7))
      (audit-native-run "native:empty-and"
       '(funcall (audit-native-compile (lambda () (and)))))
      (audit-native-run "native:global-value"
       '(funcall (audit-native-compile (lambda () audit-native-special))))
      (audit-native-run "native:special-parameter"
       '(funcall (audit-native-compile
                  (lambda (audit-native-special) (symbol-value 'audit-native-special))) 7))
      (audit-native-run "native:captured-variable"
       '(let ((x 7)) (funcall (audit-native-compile (lambda () x)))))
      (audit-native-run "native:closure-result"
       '(let ((f (funcall (audit-native-compile (lambda (x) (lambda () x))) 7)))
          (funcall f)))
      (audit-native-run "native:let-star-shadow"
       '(funcall (audit-native-compile (lambda (x) (let* ((x (1+ x))) x))) 7))
      (audit-native-run "native:error-aborts-sequence"
       '(progn
          (setq audit-native-side-effect nil)
          (let ((f (audit-native-compile
                    (lambda () (error "audit-stop") (setq audit-native-side-effect t)))))
            (list (condition-case e (funcall f) (error (car e)))
                  audit-native-side-effect))))
      (audit-native-run "native:quoted-constant-identity"
       '(let ((f (audit-native-compile (lambda () '(1 2)))))
          (eq (funcall f) (funcall f))))
      (audit-native-run "native:string-byte-kind"
       '(let ((f (audit-native-compile (lambda () "\200\377"))))
          (list (multibyte-string-p (funcall f)) (string-to-list (funcall f)))))
      (audit-native-run "native:string-byte-kind-baseline"
       '(list (multibyte-string-p "\200\377") (string-to-list "\200\377")))
      (audit-native-run "native:file-top-level-forms"
       '(let ((source (expand-file-name "top-level.el" audit-native-dir))
              (output (expand-file-name "top-level.eln" audit-native-dir)))
          (with-temp-file source
            (insert ";;; -*- lexical-binding: t -*-\n"
                    "(setq audit-native-loaded 42)\n"
                    "(defun audit-native-file-function () 7)\n"
                    "(provide 'audit-native-file-feature)\n"))
          (native-compile source output)
          (setq audit-native-loaded nil)
          (native-elisp-load output)
          (list audit-native-loaded (featurep 'audit-native-file-feature)
                (audit-native-file-function))))
      (audit-native-run "native:file-parse-error"
       '(let ((source (expand-file-name "broken.el" audit-native-dir))
              (output (expand-file-name "broken.eln" audit-native-dir)))
          (with-temp-file source
            (insert ";;; -*- lexical-binding: t -*-\n"
                    "(defun audit-native-valid-prefix () 7)\n(\n"))
          (condition-case e (progn (native-compile source output) 'compiled)
            (error (car e))))))
  (delete-directory audit-native-dir t))
(princ "AUDIT-DONE\n")
