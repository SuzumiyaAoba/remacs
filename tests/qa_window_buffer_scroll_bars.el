;;; qa_window_buffer_scroll_bars.el --- Reset nonpersistent window scroll bars -*- lexical-binding:t; -*-

(qa
 (let* ((window (selected-window))
        (original-buffer (window-buffer window))
        (reset-buffer (generate-new-buffer " *qa-scroll-reset*"))
        (keep-buffer (generate-new-buffer " *qa-scroll-keep*"))
        (persistent-buffer (generate-new-buffer " *qa-scroll-persistent*")))
   (unwind-protect
       (progn
         (dolist (buffer (list reset-buffer keep-buffer persistent-buffer))
           (with-current-buffer buffer
             (setq-local vertical-scroll-bar 'right)))

         (set-window-scroll-bars window nil 'left nil nil nil)
         (set-window-buffer window reset-buffer)
         (let ((reset (nth 2 (window-scroll-bars window))))
           (set-window-scroll-bars window nil 'left nil nil nil)
           (set-window-buffer window keep-buffer t)
           (let ((kept (nth 2 (window-scroll-bars window))))
             (set-window-scroll-bars window nil 'left nil nil t)
             (set-window-buffer window persistent-buffer)
             (list reset
                   kept
                   (nth 2 (window-scroll-bars window))
                   (nth 6 (window-scroll-bars window))))))
     (set-window-buffer window original-buffer)
     (mapc #'kill-buffer (list reset-buffer keep-buffer persistent-buffer))))
 '(right left left t))

(princ (qa--done))
