;;; -*- lexical-binding: t; -*-
(require 'filenotify)
(defun qa346-wait (predicate)
  (let ((end (+ (float-time) 3)))
    (while (and (not (funcall predicate)) (< (float-time) end))
      (read-event nil nil 0.02))
    (and (funcall predicate) t)))

(qa (let ((root (make-temp-file "/private/tmp/qa346-" t)) watch actions)
  (unwind-protect
      (progn
        (setq watch (file-notify-add-watch
                     root '(change)
                     (lambda (event)
                       (push (nth 1 event) actions)
                       (when (eq (nth 1 event) 'created) (file-notify-rm-watch (car event))))))
        (write-region "payload" nil (expand-file-name "child" root) nil 'silent)
        (qa346-wait (lambda () (memq 'created actions)))
        (list (and (memq 'created actions) t)
              (and (memq 'stopped actions) t)
              (and (file-notify-valid-p watch) t)))
    (when watch (ignore-errors (file-notify-rm-watch watch)))
    (delete-directory root t))) '(t t nil))
(princ (qa--done))
