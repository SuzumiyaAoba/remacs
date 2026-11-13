;;; qa_window_scroll_bar_size_validation.el --- Reject scroll bars that do not fit -*- lexical-binding:t; -*-

(qa
 (let* ((window (selected-window))
        (old-buffer (window-buffer window))
        (old-window-system (frame-parameter nil 'window-system))
        (old-vertical-bars (frame-parameter nil 'vertical-scroll-bars)))
   (set-frame-parameter nil 'window-system 'gpui)
   (set-frame-parameter nil 'vertical-scroll-bars 'right)
   (unwind-protect
       (progn
         (set-window-scroll-bars window nil 'left nil nil t)
         (let* ((vertical-before (window-scroll-bars window))
                (vertical-result
                 (set-window-scroll-bars window 100000 'right nil nil t))
                (vertical-after (window-scroll-bars window)))
           (set-window-scroll-bars window nil 'left nil 'bottom t)
           (let* ((horizontal-before (window-scroll-bars window))
                  (horizontal-result
                   (set-window-scroll-bars window nil 'left 100000 'bottom t))
                  (horizontal-after (window-scroll-bars window)))
             (list vertical-result
                   (equal vertical-before vertical-after)
                   horizontal-result
                   (equal horizontal-before horizontal-after)))))
     (set-window-buffer window old-buffer)
     (set-frame-parameter nil 'window-system old-window-system)
     (set-frame-parameter nil 'vertical-scroll-bars old-vertical-bars)))
 '(nil t nil t))

(princ (qa--done))
