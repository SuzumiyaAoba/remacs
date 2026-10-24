;;; audit_process_context.el --- asynchronous callback context -*- lexical-binding: t; -*-
(load (expand-file-name "tests/audit_helpers.el") nil t)

(defun audit-process-context (kind)
  (let ((owner (generate-new-buffer " *audit-process-owner*"))
        (other (generate-new-buffer " *audit-process-other*"))
        seen observed-inhibit entry-owner p)
    (unwind-protect
        (with-current-buffer owner
          (let ((callback
                 (lambda (_process _text)
                   (setq seen t observed-inhibit inhibit-quit
                         entry-owner (eq (current-buffer) owner))
                   (string-match "b" "b")
                   (set-buffer other))))
            (setq p (make-process
                     :name "audit-process-context" :noquery t
                     :connection-type 'pipe
                     :filter (if (eq kind 'filter) callback #'ignore)
                     :sentinel (if (eq kind 'sentinel) callback #'ignore)
                     :command '("/bin/sh" "-c" "printf payload"))))
          (string-match "\\(a\\)" "a")
          (let ((attempts 0))
            (while (and (not seen) (< attempts 100))
              (accept-process-output p 0.02)
              (setq attempts (1+ attempts))))
          (list seen entry-owner observed-inhibit
                (eq (current-buffer) owner)
                (condition-case err (match-string 1 "a") (error (car err)))))
      (when p (ignore-errors (delete-process p)))
      (kill-buffer owner)
      (kill-buffer other))))

(audit-check "process-context:filter" '(audit-process-context 'filter))
(audit-check "process-context:sentinel" '(audit-process-context 'sentinel))

(defun audit-process-callback-error (kind)
  (let ((debug-on-error nil) (process-error-pause-time 0)
        seen p)
    (unwind-protect
        (progn
          (let ((callback (lambda (_process _text)
                            (setq seen t) (error "audit callback error"))))
            (setq p (make-process
                     :name "audit-process-callback-error" :noquery t
                     :connection-type 'pipe
                     :filter (if (eq kind 'filter) callback #'ignore)
                     :sentinel (if (eq kind 'sentinel) callback #'ignore)
                     :command '("/bin/sh" "-c" "printf payload"))))
          (let ((outcome
                 (condition-case err
                     (progn
                       (let ((attempts 0))
                         (while (and (not seen) (< attempts 100))
                           (accept-process-output p 0.02)
                           (setq attempts (1+ attempts))))
                       'returned)
                   (error (car err)))))
            (list seen outcome)))
      (when p (ignore-errors (delete-process p))))))

(audit-check "process-context:filter-error" '(audit-process-callback-error 'filter))
(audit-check "process-context:sentinel-error" '(audit-process-callback-error 'sentinel))

(princ "AUDIT-DONE\n")
