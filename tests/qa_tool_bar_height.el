;;; The tool-bar height getter reflects allocated graphical toolbar rows.
(require 'tool-bar)

(qa (progn
      (set-frame-parameter nil 'window-system 'gpui)
      (set-frame-parameter nil 'char-height 23)
      (set-frame-parameter nil 'tool-bar-lines 1)
      (list (tool-bar-height) (tool-bar-height nil t)))
    '(1 23))

(qa (progn
      (set-frame-parameter nil 'tool-bar-lines 0)
      (list (tool-bar-height) (tool-bar-height nil t)))
    '(0 0))

(qa (progn
      (set-frame-parameter nil 'window-system nil)
      (set-frame-parameter nil 'tool-bar-lines 1)
      (list (tool-bar-height) (tool-bar-height nil t)))
    '(0 0))

(qa (progn
      (set-frame-parameter nil 'window-system nil)
      (tool-bar-height "a"))
    0)

(princ (qa--done))
