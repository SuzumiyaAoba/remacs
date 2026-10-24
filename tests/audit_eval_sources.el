;;; audit_eval_sources.el --- evaluation context, ranges, and output -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(defvar audit-eval-events nil)
(defvar audit-eval-result nil)

(audit-check "eval-buffer:target-buffer"
 '(let ((a (generate-new-buffer " *audit-eval-caller*"))
        (b (generate-new-buffer " *audit-eval-source*")))
    (unwind-protect
        (progn
          (with-current-buffer b (insert "(setq-local audit-eval-local 'target)"))
          (with-current-buffer a
            (eval-buffer b)
            (list (eq (current-buffer) a)
                  (local-variable-p 'audit-eval-local a)
                  (local-variable-p 'audit-eval-local b))))
      (kill-buffer a) (kill-buffer b))))

(dolist (entry '((eval-buffer (eval-buffer))
                 (eval-region (eval-region nil nil))))
  (audit-check (format "%s:narrowing" (car entry))
   `(let ((audit-eval-events nil))
      (with-temp-buffer
        (insert "(push 'outside audit-eval-events)\n(push 'inside audit-eval-events)")
        (goto-char 1) (forward-line 1) (narrow-to-region (point) (point-max))
        ,(cadr entry)
        (nreverse audit-eval-events)))))

(audit-check "eval-region:inaccessible-bounds"
 '(let ((audit-eval-events nil))
    (with-temp-buffer
      (insert "(push 'outside audit-eval-events)\n(push 'inside audit-eval-events)")
      (let ((end (point-max)))
        (goto-char 1) (forward-line 1) (narrow-to-region (point) end)
        (list (condition-case err (progn (eval-region 1 end) 'evaluated) (error (car err)))
              (nreverse audit-eval-events))))))

(dolist (entry '((eval-buffer (eval-buffer nil nil))
                 (eval-region (eval-region (point-min) (point-max) nil))))
  (audit-check (format "%s:discard-output" (car entry))
   `(let ((out (generate-new-buffer " *audit-eval-output*")))
      (unwind-protect
          (let ((standard-output out))
            (with-temp-buffer (insert "(princ \"payload\")") ,(cadr entry))
            (with-current-buffer out (buffer-string)))
        (kill-buffer out)))))

(dolist (entry '((eval-buffer (eval-buffer nil target))
                 (eval-region (eval-region (point-min) (point-max) target))))
  (audit-check (format "%s:explicit-output" (car entry))
   `(let ((outer (generate-new-buffer " *audit-eval-outer*"))
          (target (generate-new-buffer " *audit-eval-target*")))
      (unwind-protect
          (let ((standard-output outer))
            (with-temp-buffer (insert "(princ \"payload\")") ,(cadr entry))
            (list (with-current-buffer outer (buffer-string))
                  (with-current-buffer target (buffer-string))))
        (kill-buffer outer) (kill-buffer target)))))

(audit-check "eval-region:read-function"
 '(let ((reads 0))
    (with-temp-buffer
      (insert "(+ 1 2)")
      (eval-region (point-min) (point-max) nil
                   (lambda (stream) (setq reads (1+ reads)) (read stream)))
      (> reads 0))))

(audit-check "eval-buffer:lexical-header"
 '(let ((audit-eval-result nil))
    (with-temp-buffer
      (insert ";;; -*- lexical-binding: t -*-\n"
              "(setq audit-eval-result (let ((x 7)) (lambda () x)))")
      (let ((lexical-binding nil)) (eval-buffer))
      (condition-case err (funcall audit-eval-result) (error (car err))))))

(dolist (entry '((eval-buffer (eval-buffer))
                 (eval-region (eval-region (point-min) (point-max)))))
  (audit-check (format "%s:return-value" (car entry))
   `(with-temp-buffer (insert "42") ,(cadr entry))))

(princ "AUDIT-DONE\n")
