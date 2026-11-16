;;; Frame text dimensions use pixels, with rows and columns available separately.
(require 'frame)

(qa (progn
      (set-frame-parameter nil 'window-system 'gpui)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'char-width 8)
      (set-frame-parameter nil 'char-height 20)
      (set-frame-parameter nil 'tool-bar-lines 1)
      (set-frame-parameter nil 'tab-bar-lines 1)
      (list (frame-text-width) (frame-text-height)
            (frame-text-cols) (frame-text-lines)))
    '(640 460 80 23))

(qa (progn
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (list (frame-text-width) (frame-text-height)
            (frame-text-cols) (frame-text-lines)))
    '(640 500 80 25))

(qa (progn
      (set-frame-parameter nil 'window-system nil)
      (set-frame-parameter nil 'char-width nil)
      (set-frame-parameter nil 'char-height nil)
      (list (frame-text-width) (frame-text-height)
            (frame-text-cols) (frame-text-lines)))
    '(80 25 80 25))

(princ (qa--done))
