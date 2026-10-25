;;; -*- lexical-binding: nil; -*-
;; window-end does not infer a rendered end from window height (#511).
(qa (with-temp-buffer
      (dotimes (_ 50) (insert "x\n"))
      (set-window-buffer (selected-window) (current-buffer))
      (set-window-start (selected-window) 1 t)
      (list (window-height)
            (window-end nil nil)
            (window-end nil t)
            (point-max)))
    '(24 101 101 101))
(princ (qa--done))
