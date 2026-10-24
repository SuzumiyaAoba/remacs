;;; audit_process_callbacks.el --- callback error containment -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "process:filter-error-containment"
 '(let ((process-error-pause-time 0) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "audit-error-filter" :noquery t
                               :command '("/bin/sh" "-c" "printf payload")
                               :sentinel #'ignore
                               :filter (lambda (_p _text) (error "audit-filter-error"))))
          (sleep-for 0.1)
          (condition-case err (progn (accept-process-output p 0.1) 'returned)
            (error (car err))))
      (when p (ignore-errors (delete-process p))))))

(audit-check "process:sentinel-error-containment"
 '(let ((process-error-pause-time 0) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "audit-error-sentinel" :noquery t
                               :command '("/bin/sh" "-c" "exit 0")
                               :sentinel (lambda (_p _text) (error "audit-sentinel-error"))))
          (condition-case err
              (progn (sleep-for 0.2) (accept-process-output p 0.1) 'returned)
            (error (car err))))
      (when p (ignore-errors (delete-process p))))))

(princ "AUDIT-DONE\n")
