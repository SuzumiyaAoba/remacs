;;; -*- lexical-binding: nil; -*-
;; Flymake mode state is local to the current buffer (#578).
(require 'flymake)
(qa
 (with-temp-buffer
   (flymake-mode 1)
   (let ((outer-local (local-variable-p 'flymake-mode))
         (outer-value flymake-mode))
     (with-temp-buffer
       (list outer-local outer-value
             (local-variable-p 'flymake-mode) flymake-mode))))
 '(t t nil nil))
(princ (qa--done))
