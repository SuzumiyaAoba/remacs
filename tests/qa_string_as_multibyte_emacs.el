;;; -*- lexical-binding: nil; -*-
;; Canonical Emacs UTF-8 includes surrogates and high Mule (#427).
(dolist (code '(0 127 128 255 256 #x7ff #x800 #xd7ff #xd800 #xdbff
               #xdc00 #xdfff #xe000 #xffff #x10000 #x10ffff #x110000
               #x1fffff #x200000 #x3fff7f))
  (let* ((original (string code))
         (bytes (string-as-unibyte original))
         (source (propertize bytes 'tag 'value))
         (result (string-as-multibyte source)))
    (qa (string-to-list result) (list code))
    (qa (multibyte-string-p result) t)
    (qa (eq result source) nil)
    (qa (text-properties-at 0 result) nil)
    (qa (get-text-property 0 'tag source) 'value)
    (qa (string-to-list (string-as-unibyte result)) (string-to-list bytes))))
;; Any isolated high byte is preserved as byte8, never replaced by U+FFFD.
(dotimes (byte 256)
  (let ((result (string-as-multibyte (unibyte-string byte))))
    (qa (string-to-list result) (list (if (< byte 128) byte (+ #x3fff00 byte))))
    (qa (multibyte-string-p result) t)))
;; Overlong, incomplete, reserved byte8, and out-of-range encodings.
(dolist (sequence '((192 128) (193 191) (224 128 128) (240 128 128 128)
                    (248 128 128 128 128) (248 143 191 190 128)
                    (248 144 128 128 128) (249 128 128 128 128)
                    (194) (226 130) (240 159 152) (248 136 128 128)
                    (226 40 161) (255)))
  (let* ((bytes (apply #'unibyte-string sequence))
         (expected (mapcar (lambda (b) (if (< b 128) b (+ #x3fff00 b))) sequence))
         (mixed (concat (unibyte-string 65) bytes
                        (string-as-unibyte (string #x200000)) (unibyte-string 90))))
    (qa (string-to-list (string-as-multibyte bytes)) expected)
    (qa (string-to-list (string-as-multibyte mixed))
        (append '(65) expected '(2097152 90)))))
(princ (qa--done))
