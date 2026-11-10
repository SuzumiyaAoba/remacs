;;; qa_pcvs_scroll_target.el --- PCL-CVS scroll target regression -*- lexical-binding:t -*-

(require 'pcvs)

(qa
 (let* ((source (generate-new-buffer " *qa-pcvs-source*"))
        (file (generate-new-buffer " *qa-pcvs-file*"))
        (cvs-buffer-name-alist
         '(("qa-status" "*qa-pcvs-status*" fundamental-mode)))
        (cvs-temp-buffers nil)
        cvs-buffer
        start-window other-window)
   (unwind-protect
       (progn
         (delete-other-windows)
         (set-window-buffer (selected-window) source)
         (setq start-window (selected-window)
               other-window (split-window-right))
         (with-current-buffer source
           (setq default-directory temporary-file-directory
                 cvs-temp-buffer nil)
           (setq cvs-buffer (cvs-temp-buffer "qa-status")))
         ;; Make the CVS output the target in the other window, even if the
         ;; window selection policy chose a different placement.
         (set-window-buffer start-window source)
         (set-window-buffer other-window cvs-buffer)
         (let ((before (with-current-buffer source
                         (list (eq other-window-scroll-buffer cvs-buffer)
                               (eq cvs--other-window-scroll-buffer cvs-buffer)))))
           (with-selected-window other-window
             (switch-to-buffer file))
           ;; Window changes are normally reported during redisplay.  Run the
           ;; registered hook to make that notification deterministic here.
           (run-hook-with-args 'window-buffer-change-functions
                               (selected-frame))
           (with-current-buffer source
             (list before
                   (local-variable-p 'other-window-scroll-buffer)
                   cvs--other-window-scroll-buffer))))
     (delete-other-windows)
     (when (buffer-live-p source) (kill-buffer source))
     (when (buffer-live-p file) (kill-buffer file))
     (when (buffer-live-p cvs-buffer) (kill-buffer cvs-buffer))))
 '((t t) nil nil))

(princ (qa--done))
