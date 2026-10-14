;;; -*- lexical-binding: nil; -*-
;; Issue #74: successful calls return the exact minibuffer window.
(let ((mini (minibuffer-window))
      (selected (selected-window))
      (buffer (current-buffer)))
  (unwind-protect
      (progn
        (qa (window-minibuffer-p mini) t)
        (qa-same (set-minibuffer-window mini) mini)
        (qa-same (set-minibuffer-window mini) mini)
        (qa-same (minibuffer-window) mini)
        (qa-same (selected-window) selected)
        (qa-same (current-buffer) buffer)
        (qa (minibuffer-depth) 0)
        (qa (active-minibuffer-window) nil)
        (qa-err-data (set-minibuffer-window selected) 'error '("Window is not a minibuffer window"))
        (qa-same (set-minibuffer-window mini) mini)
        (dolist (value (list nil t 1 1.5 "window" [] (list 'a) buffer))
          (qa-err-data (set-minibuffer-window value) 'wrong-type-argument (list 'windowp value)))
        (qa-same (selected-window) selected)
        (qa-same (current-buffer) buffer))
    (set-minibuffer-window mini)))
(save-window-excursion
  (let ((other (split-window-below)))
    (qa-err-data (set-minibuffer-window other) 'error '("Window is not a minibuffer window"))
    (delete-window other)
    (qa-err-data (set-minibuffer-window other) 'error '("Window is not a minibuffer window"))))
(princ (qa--done))
