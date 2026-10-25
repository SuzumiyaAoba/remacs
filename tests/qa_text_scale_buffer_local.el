;;; -*- lexical-binding: nil; -*-
;; Text scale state is buffer-local after loading face-remap (#587).
(require 'face-remap)
(qa
 (with-temp-buffer
   (text-scale-set 2)
   (setq text-scale-remap-header-line t)
   (let ((outer (list text-scale-mode-amount
                      (local-variable-p 'text-scale-mode-amount)
                      text-scale-remap-header-line
                      (local-variable-p 'text-scale-remap-header-line))))
     (with-temp-buffer
       (list outer
             text-scale-mode-amount
             (local-variable-p 'text-scale-mode-amount)
             text-scale-remap-header-line
             (local-variable-p 'text-scale-remap-header-line)))))
 '((2 t t t) 0 nil nil nil))
(princ (qa--done))
