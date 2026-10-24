;;; -*- lexical-binding: t; -*-

;; 'library:text-scale-after-autoload'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(progn (with-temp-buffer (text-scale-increase 1)) (and (assq 'text-scale-mode minor-mode-alist) t)) t))
           (error (list 'error err))))
  (terpri))

;; 'library:text-scale-after-require'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(progn (require 'face-remap) (list (and (assq 'text-scale-mode minor-mode-alist) t) (and (assq 'buffer-face-mode minor-mode-alist) t))) t))
           (error (list 'error err))))
  (terpri))
