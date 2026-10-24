;;; audit_filenotify_lifecycle.el --- notification lifecycle probes -*- lexical-binding: t; -*-
(load (expand-file-name "tests/audit_helpers.el") nil t)
(require 'filenotify)

(defun audit-notify-wait (predicate)
  (let ((tries 0))
    (while (and (not (funcall predicate)) (< tries 100))
      (read-event nil nil 0.02)
      (setq tries (1+ tries)))
    (funcall predicate)))

(defun audit-notify-directory-case (remove)
  (let ((dir (make-temp-file "remacs-notify-" t))
        watch actions)
    (unwind-protect
        (progn
          (setq watch
                (file-notify-add-watch
                 dir '(change)
                 (lambda (event)
                   (push (nth 1 event) actions)
                   (when (and remove (eq (nth 1 event) 'created))
                     (file-notify-rm-watch (car event))))))
          (write-region "payload" nil (expand-file-name "child" dir) nil 'silent)
          (audit-notify-wait (lambda () (memq 'created actions)))
          (list (and (memq 'created actions) t)
                (and (memq 'stopped actions) t)
                (file-notify-valid-p watch)))
      (when watch (ignore-errors (file-notify-rm-watch watch)))
      (delete-directory dir t))))

(audit-check "notify:directory-control" '(audit-notify-directory-case nil))
(audit-check "notify:directory-remove-in-callback" '(audit-notify-directory-case t))

(defun audit-notify-rearm (keep-another-watch)
  (let ((file (make-temp-file "remacs-notify-")) watch anchor actions)
    (unwind-protect
        (progn
          (when keep-another-watch
            (setq anchor (file-notify-add-watch file '(change) #'ignore)))
          (setq watch (file-notify-add-watch
                       file '(change) (lambda (e) (push (nth 1 e) actions))))
          (write-region "first" nil file nil 'silent)
          (audit-notify-wait (lambda () (memq 'changed actions)))
          (let ((first (and (memq 'changed actions) t)))
            (file-notify-rm-watch watch)
            ;; Allow the reader to observe the closed kqueue before reopening.
            (sleep-for 0.1)
            (setq actions nil
                  watch (file-notify-add-watch
                         file '(change) (lambda (e) (push (nth 1 e) actions))))
            (write-region "second" nil file nil 'silent)
            (audit-notify-wait (lambda () (memq 'changed actions)))
            (list first (and (memq 'changed actions) t)
                  (file-notify-valid-p watch))))
      (when watch (ignore-errors (file-notify-rm-watch watch)))
      (when anchor (ignore-errors (file-notify-rm-watch anchor)))
      (delete-file file))))

(audit-check "notify:file-rearm" '(audit-notify-rearm nil))
(audit-check "notify:file-rearm-keep-another-control" '(audit-notify-rearm t))

(defun audit-notify-error-queue (fail-first)
  (let ((first (make-temp-file "remacs-notify-first-"))
        (second (make-temp-file "remacs-notify-second-"))
        watch1 watch2 seen (errors 0))
    (unwind-protect
        (progn
          (setq watch1
                (file-notify-add-watch
                 first '(change)
                 (lambda (event)
                   (when (eq (nth 1 event) 'changed)
                     (push 'first seen)
                     (when fail-first (error "audit notification callback"))))))
          (setq watch2
                (file-notify-add-watch
                 second '(change)
                 (lambda (event)
                   (when (eq (nth 1 event) 'changed) (push 'second seen)))))
          (write-region "first" nil first nil 'silent)
          ;; Deliberately avoid any event pump while the reader queues changes.
          (let ((end (+ (float-time) 0.1)))
            (while (< (float-time) end)))
          (write-region "second" nil second nil 'silent)
          (let ((end (+ (float-time) 0.1)))
            (while (< (float-time) end)))
          (let ((tries 0))
            (while (and (not (memq 'second seen)) (< tries 100))
              (condition-case nil (read-event nil nil 0.02)
                (error (setq errors (1+ errors))))
              (setq tries (1+ tries))))
          (list (and (memq 'first seen) t)
                (and (memq 'second seen) t)
                (> errors 0)))
      (when watch1 (ignore-errors (file-notify-rm-watch watch1)))
      (when watch2 (ignore-errors (file-notify-rm-watch watch2)))
      (delete-file first)
      (delete-file second))))

(audit-check "notify:callback-error-queue" '(audit-notify-error-queue t))
(audit-check "notify:callback-success-queue-control" '(audit-notify-error-queue nil))

(princ "AUDIT-DONE\n")
