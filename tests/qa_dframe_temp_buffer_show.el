;;; -*- lexical-binding: t; -*-
;; dframe keeps the buffer-taking temporary display callback intact (#1974).

(require 'dframe)

(qa
 (let ((shown nil)
       (hook-ran nil)
       (buffer (get-buffer-create "*dframe temp show test*")))
   (unwind-protect
       (with-temp-buffer
         (let ((callback (lambda (buffer) (setq shown buffer)))
               (hook (list (lambda () (setq hook-ran t)))))
           (setq-local temp-buffer-show-function callback)
           (setq-local temp-buffer-show-hook hook)
           (dframe--install-temp-buffer-show-function)
           (let ((dframe-attached-frame nil))
             (dframe-temp-buffer-show-function buffer))
           (let ((saved dframe--saved-temp-buffer-show-function))
             ;; Reinitializing the dframe wrapper must not save itself.
             (dframe--install-temp-buffer-show-function)
             (list (eq shown buffer)
                   hook-ran
                   (eq temp-buffer-show-function
                       #'dframe-temp-buffer-show-function)
                   (eq saved dframe--saved-temp-buffer-show-function)
                   (eq hook temp-buffer-show-hook)))))
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 '(t nil t t t))

(princ (qa--done))
