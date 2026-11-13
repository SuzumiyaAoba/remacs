;;; qa_scroll_bar_zero_disables_type.el --- Zero scroll bar dimensions disable bars -*- lexical-binding:t; -*-

(qa
 (let ((window (selected-window)))
   (set-frame-parameter nil 'window-system 'gpui)
   (set-frame-parameter nil 'vertical-scroll-bars 'right)
   (set-frame-parameter nil 'horizontal-scroll-bars 'bottom)
   (set-window-scroll-bars window nil 'right nil 'bottom t)
   (list (set-window-scroll-bars window 0 'right 0 'bottom t)
         (window-current-scroll-bars window)))
 '(t (nil . nil)))

(princ (qa--done))
