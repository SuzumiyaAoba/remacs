;;; -*- lexical-binding: t; -*-
;; Remacs regression for #1306.  GNU 31.1 still flushes a cookie-bearing
;; rename-from when an unrelated event arrives.
(require 'filenotify)

(when qa--remacs
  (qa
   (let* ((events nil)
          (watch (file-notify--watch-make
                  "/tmp/" nil (lambda (event) (push event events))))
          (file-notify-descriptors (make-hash-table :test #'equal))
          (file-notify--pending-rename nil))
     (puthash 'audit watch file-notify-descriptors)
     (file-notify--handle-event 'audit '(renamed-from) "old" 7)
     (file-notify--handle-event 'audit '(created) "other" nil)
     (file-notify--handle-event 'audit '(renamed-to) "new" 7)
     (nreverse events))
   '((audit created "/tmp/other")
     (audit renamed "/tmp/old" "/tmp/new")))
  ;; Preserve the old fallback when a backend has no rename cookie.
  (qa
   (let* ((events nil)
          (watch (file-notify--watch-make
                  "/tmp/" nil (lambda (event) (push event events))))
          (file-notify-descriptors (make-hash-table :test #'equal))
          (file-notify--pending-rename nil))
     (puthash 'audit watch file-notify-descriptors)
     (file-notify--handle-event 'audit '(renamed-from) "old" nil)
     (file-notify--handle-event 'audit '(created) "other" nil)
     (nreverse events))
   '((audit deleted "/tmp/old")
     (audit created "/tmp/other"))))

(princ (qa--done)) (terpri)
