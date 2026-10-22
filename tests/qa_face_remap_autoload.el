;;; qa_face_remap_autoload.el --- Face remapping autoload registration -*- lexical-binding: t; -*-

(qa (featurep 'face-remap) nil)
(qa (list (assq 'text-scale-mode minor-mode-alist)
          (assq 'buffer-face-mode minor-mode-alist))
    '(nil nil))
(qa (autoloadp (symbol-function 'text-scale-increase)) t)
(qa (with-temp-buffer
      (text-scale-increase 1)
      (list text-scale-mode text-scale-mode-amount))
    '(t 1))
(qa (featurep 'face-remap) t)
(qa (list (assq 'text-scale-mode minor-mode-alist)
          (assq 'buffer-face-mode minor-mode-alist))
    '((text-scale-mode (" " text-scale-mode-lighter))
      (buffer-face-mode " BufFace")))
(qa (progn
      (require 'face-remap)
      (mapcar (lambda (mode)
                (length (delq nil (mapcar (lambda (entry) (eq (car entry) mode))
                                         minor-mode-alist))))
              '(text-scale-mode buffer-face-mode)))
    '(1 1))
(qa (list (assq 'text-scale-mode minor-mode-map-alist)
          (assq 'buffer-face-mode minor-mode-map-alist))
    '(nil nil))
(qa (with-temp-buffer
      (buffer-face-mode 1)
      (list buffer-face-mode (and face-remapping-alist t)))
    '(t t))

(princ (qa--done))
