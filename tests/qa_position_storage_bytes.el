;;; -*- lexical-binding: nil; -*-
;; Emacs storage bytes are distinct from Rust proxy UTF-8 (#622).
(defun qa622-convert (codes narrowed unibyte)
  (with-temp-buffer
    (when unibyte (set-buffer-multibyte nil))
    (insert (apply #'string codes))
    (when narrowed (narrow-to-region 2 (1- (point-max))))
    (list (buffer-size) (point-min) (point-max)
          (mapcar #'position-bytes (number-sequence 0 (+ 2 (buffer-size))))
          (mapcar #'byte-to-position (number-sequence 0 40)))))
(qa (qa622-convert '(97 233 26085 128512 120) nil nil) '(5 1 6 (nil 1 2 4 7 11 12 nil) (nil 1 2 2 3 3 3 4 4 4 4 5 6 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(97 233 26085 128512 120) t nil) '(5 2 5 (nil 1 2 4 7 11 12 nil) (nil 1 2 2 3 3 3 4 4 4 4 5 6 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(55296 1114112 2097152 4194176 120) nil nil) '(5 1 6 (nil 1 4 8 13 15 16 nil) (nil 1 1 1 2 2 2 2 3 3 3 3 3 4 4 5 6 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(55296 1114112 2097152 4194176 120) t nil) '(5 2 5 (nil 1 4 8 13 15 16 nil) (nil 1 1 1 2 2 2 2 3 3 3 3 3 4 4 5 6 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(57343 2097151 4194175 4194303 120) nil nil) '(5 1 6 (nil 1 4 8 13 15 16 nil) (nil 1 1 1 2 2 2 2 3 3 3 3 3 4 4 5 6 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(57343 2097151 4194175 4194303 120) t nil) '(5 2 5 (nil 1 4 8 13 15 16 nil) (nil 1 1 1 2 2 2 2 3 3 3 3 3 4 4 5 6 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(127 128 2047 2048 65535 65536) nil nil) '(6 1 7 (nil 1 2 4 6 9 12 16 nil) (nil 1 2 2 3 3 4 4 4 5 5 5 6 6 6 6 7 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(127 128 2047 2048 65535 65536) t nil) '(6 2 6 (nil 1 2 4 6 9 12 16 nil) (nil 1 2 2 3 3 4 4 4 5 5 5 6 6 6 6 7 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(65 128 255 66) nil t) '(4 1 5 (nil 1 2 3 4 5 nil) (nil 1 2 3 4 5 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))
(qa (qa622-convert '(65 128 255 66) t t) '(4 2 4 (nil 1 2 3 4 5 nil) (nil 1 2 3 4 5 nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)))

(princ (qa--done))
