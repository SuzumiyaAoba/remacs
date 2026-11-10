;;; -*- lexical-binding: t; -*-
;; Exiting a secondary calendar must leave the standard calendar visible (#1966).

(require 'calendar)

(qa
 (let* ((calendar-buffer "*Calendar*")
        (main (get-buffer-create calendar-buffer))
        (secondary (get-buffer-create "*Secondary Calendar*"))
        (holidays (get-buffer-create "*Holidays*"))
        (saved (current-window-configuration)))
   (unwind-protect
       (progn
         (with-current-buffer main (calendar-mode))
         (with-current-buffer secondary (calendar-mode))
         (delete-other-windows)
         (set-window-buffer (selected-window) main)
         (let ((secondary-window (split-window-right)))
           (set-window-buffer secondary-window secondary)
           (let ((holiday-window (split-window-below)))
             (set-window-buffer holiday-window holidays)
             (select-window secondary-window)
             (calendar-exit)
             (let ((secondary-state
                    (list (and (buffer-live-p main)
                               (get-buffer-window main)
                               t)
                          (and (get-buffer-window holidays) t)
                          (and (get-buffer-window secondary) t))))
               (select-window (get-buffer-window main))
               (calendar-exit)
               (list secondary-state
                     (and (get-buffer-window main) t)
                     (and (get-buffer-window holidays) t))))))
     (set-window-configuration saved)
     (dolist (buffer (list main secondary holidays))
       (when (buffer-live-p buffer)
         (kill-buffer buffer)))))
 '((t t nil) nil nil))

(princ (qa--done))
