;;; -*- lexical-binding: t; -*-
(require 'filenotify)
(defun qa348-wait (predicate)
  (let ((end (+ (float-time) 3)))
    (while (and (not (funcall predicate)) (< (float-time) end))
      (read-event nil nil 0.02))
    (and (funcall predicate) t)))
(defun qa348-rearm (keep-guardian)
  (let* ((root (make-temp-file "/private/tmp/qa348-" t))
         (file (expand-file-name "data" root))
         (guard-file (expand-file-name "guard" root))
         watch guardian actions results)
    (unwind-protect
        (progn
          (write-region "start" nil file nil 'silent)
          (when keep-guardian
            (write-region "guard" nil guard-file nil 'silent)
            (setq guardian (file-notify-add-watch guard-file '(change) #'ignore)))
          (dotimes (round 3)
            (setq actions nil
                  watch (file-notify-add-watch file '(change)
                          (lambda (event) (push (nth 1 event) actions))))
            (write-region (format "round-%s" round) nil file nil 'silent)
            (push (list (qa348-wait (lambda () (memq 'changed actions)))
                        (and (file-notify-valid-p watch) t)) results)
            (file-notify-rm-watch watch)
            (setq watch nil)
            (sleep-for 0.15))
          (nreverse results))
      (dolist (descriptor (list watch guardian))
        (when descriptor (ignore-errors (file-notify-rm-watch descriptor))))
      (delete-directory root t))))

(qa (let ((file (make-temp-file "/private/tmp/qa348-delete-" nil nil "start"))
      watch actions)
  (unwind-protect
      (progn
        (setq watch (file-notify-add-watch file '(change)
                      (lambda (event) (push (nth 1 event) actions))))
        (delete-file file)
        (let ((stopped (qa348-wait (lambda () (memq 'stopped actions))))
              (old-valid (and (file-notify-valid-p watch) t)))
          (sleep-for 0.15)
          (write-region "recreated" nil file nil 'silent)
          (setq actions nil
                watch (file-notify-add-watch file '(change)
                        (lambda (event) (push (nth 1 event) actions))))
          (write-region "changed" nil file nil 'silent)
          (list stopped old-valid
                (qa348-wait (lambda () (memq 'changed actions)))
                (and (file-notify-valid-p watch) t))))
    (when watch (ignore-errors (file-notify-rm-watch watch)))
    (when (file-exists-p file) (delete-file file)))) '(t nil t t))
(princ (qa--done))
