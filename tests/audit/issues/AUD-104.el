;;; -*- lexical-binding: t; -*-
(defvar audit-eval-events nil)
(defvar audit-eval-result nil)
;; 'eval-buffer:discard-output'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((out (generate-new-buffer " *audit-eval-output*"))) (unwind-protect (let ((standard-output out)) (with-temp-buffer (insert "(princ \"payload\")") (eval-buffer nil nil)) (with-current-buffer out (buffer-string))) (kill-buffer out))) t))
           (error (list 'error err))))
  (terpri))

;; 'eval-region:discard-output'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((out (generate-new-buffer " *audit-eval-output*"))) (unwind-protect (let ((standard-output out)) (with-temp-buffer (insert "(princ \"payload\")") (eval-region (point-min) (point-max) nil)) (with-current-buffer out (buffer-string))) (kill-buffer out))) t))
           (error (list 'error err))))
  (terpri))

;; 'eval-buffer:explicit-output'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((outer (generate-new-buffer " *audit-eval-outer*")) (target (generate-new-buffer " *audit-eval-target*"))) (unwind-protect (let ((standard-output outer)) (with-temp-buffer (insert "(princ \"payload\")") (eval-buffer nil target)) (list (with-current-buffer outer (buffer-string)) (with-current-buffer target (buffer-string)))) (kill-buffer outer) (kill-buffer target))) t))
           (error (list 'error err))))
  (terpri))

;; 'eval-region:explicit-output'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((outer (generate-new-buffer " *audit-eval-outer*")) (target (generate-new-buffer " *audit-eval-target*"))) (unwind-protect (let ((standard-output outer)) (with-temp-buffer (insert "(princ \"payload\")") (eval-region (point-min) (point-max) target)) (list (with-current-buffer outer (buffer-string)) (with-current-buffer target (buffer-string)))) (kill-buffer outer) (kill-buffer target))) t))
           (error (list 'error err))))
  (terpri))
