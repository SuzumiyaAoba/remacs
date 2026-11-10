;;; -*- lexical-binding: t; -*-
;; A show function that runs temp-buffer-show-hook must not recurse in dframe (#2851).

(require 'dframe)

(qa
 (let ((shown nil)
       (show-count 0)
       (hook-count 0)
       (buffer (get-buffer-create "*dframe show recursion test*")))
   (unwind-protect
       (with-temp-buffer
         (let ((callback
                (lambda (&optional shown-buffer)
                  (setq show-count (1+ show-count))
                  (when (> show-count 1)
                    (error "Recursive temporary buffer show"))
                  (when shown-buffer
                    (setq shown shown-buffer))
                  (run-hooks 'temp-buffer-show-hook)))
               (hook (list (lambda () (setq hook-count (1+ hook-count))))))
           (setq-local temp-buffer-show-function callback)
           (setq-local temp-buffer-show-hook hook)
           (dframe--install-temp-buffer-show-function)
           (let ((dframe-attached-frame nil))
             (dframe-temp-buffer-show-function buffer))
           (list (eq shown buffer)
                 show-count
                 hook-count
                 (eq temp-buffer-show-hook hook))))
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 '(t 1 1 t))

(princ (qa--done))
