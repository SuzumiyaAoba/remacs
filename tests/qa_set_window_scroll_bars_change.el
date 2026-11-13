;;; qa_set_window_scroll_bars_change.el --- Report changed scroll bar settings -*- lexical-binding:t; -*-

(qa
 (let ((window (selected-window)))
   (set-frame-parameter nil 'window-system 'gpui)
   (set-frame-parameter nil 'vertical-scroll-bars 'right)
   (let* ((changed (set-window-scroll-bars window nil 'left nil nil t))
          (unchanged (set-window-scroll-bars window nil 'left nil nil t))
          (persistence-only
           (set-window-scroll-bars window nil 'left nil nil nil)))
     (list changed unchanged persistence-only)))
 '(t nil nil))

(princ (qa--done))
