;;; -*- lexical-binding: t; -*-
(require 'filenotify)
(defun qa346-wait (predicate)
  (let ((end (+ (float-time) 3)))
    (while (and (not (funcall predicate)) (< (float-time) end))
      (read-event nil nil 0.02))
    (and (funcall predicate) t)))

(qa (let ((root (make-temp-file "/private/tmp/qa346-" t))
      guardian watch replacement old-actions new-events)
  (unwind-protect
      (progn
        (write-region "guard" nil (expand-file-name "guard" root) nil 'silent)
        (setq guardian (file-notify-add-watch (expand-file-name "guard" root)
                                             '(change) #'ignore))
        (setq watch
              (file-notify-add-watch root '(change)
                (lambda (event)
                  (push (nth 1 event) old-actions)
                  (when (eq (nth 1 event) 'created)
                    (file-notify-rm-watch (car event))
                    (setq replacement
                          (file-notify-add-watch root '(change)
                            (lambda (next) (push next new-events))))))))
        (write-region "first" nil (expand-file-name "child1" root) nil 'silent)
        (qa346-wait (lambda () replacement))
        (let ((stale (seq-some
                      (lambda (event)
                        (equal (file-name-nondirectory (nth 2 event)) "child1"))
                      new-events)))
          (write-region "second" nil (expand-file-name "child2" root) nil 'silent)
          (list (and (memq 'created old-actions) t)
                (and (memq 'stopped old-actions) t)
                (and stale t)
                (qa346-wait
                 (lambda () (seq-some
                        (lambda (event)
                          (and (eq (nth 1 event) 'created)
                               (equal (file-name-nondirectory (nth 2 event)) "child2")))
                        new-events)))
                (and (file-notify-valid-p replacement) t))))
    (dolist (descriptor (list watch replacement guardian))
      (when descriptor (ignore-errors (file-notify-rm-watch descriptor))))
    (delete-directory root t))) '(t t t t t))
(princ (qa--done))
