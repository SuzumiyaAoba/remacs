;;; -*- lexical-binding: t; -*-
(require 'filenotify)
(defun qa354-spin ()
  ;; Give the kernel reader time without dispatching Lisp events.
  (let ((end (+ (float-time) 0.1)))
    (while (< (float-time) end))))
(defun qa354-files (error-enabled)
  (let* ((root (make-temp-file "/private/tmp/qa354-" t))
         (first (expand-file-name "first" root))
         (second (expand-file-name "second" root))
         watch1 watch2 seen (errors 0))
    (unwind-protect
        (progn
          (write-region "start" nil first nil 'silent)
          (write-region "start" nil second nil 'silent)
          (setq watch1
                (file-notify-add-watch first '(change)
                  (lambda (event)
                    (when (eq (nth 1 event) 'changed)
                      (push 'first seen)
                      (when error-enabled
                        (garbage-collect)
                        (error "qa354 callback"))))))
          (setq watch2
                (file-notify-add-watch second '(change)
                  (lambda (event)
                    (when (eq (nth 1 event) 'changed) (push 'second seen)))))
          (write-region "first" nil first nil 'silent)
          (qa354-spin)
          (write-region "second" nil second nil 'silent)
          (qa354-spin)
          (let ((end (+ (float-time) 3)))
            (while (and (not (and (memq 'first seen) (memq 'second seen)))
                        (< (float-time) end))
              (condition-case nil (read-event nil nil 0.02)
                (error (setq errors (1+ errors))))))
          (list (and (memq 'first seen) t)
                (and (memq 'second seen) t) (> errors 0)))
      (dolist (watch (list watch1 watch2))
        (when watch (ignore-errors (file-notify-rm-watch watch))))
      (delete-directory root t))))

(qa (qa354-files t) '(t t t))
(princ (qa--done))
