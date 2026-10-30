;;; qa_vconcat_unibyte.el --- Raw byte sequence elements -*- lexical-binding: nil; -*-

(qa (vconcat "\x80x") [128 120])
(qa (vconcat (unibyte-string 0 127 128 255)) [0 127 128 255])
;; Cover every byte, including NUL, controls, high bytes and UTF-8-looking pairs.
(dotimes (byte 256)
  (qa (vconcat (unibyte-string byte)) (vector byte)))
(let* ((bytes (number-sequence 0 255))
       (string (apply #'unibyte-string bytes)))
  (qa (vconcat string) (apply #'vector bytes))
  (qa (vconcat (copy-sequence string)) (apply #'vector bytes))
  ;; Substrings preserve the source unibyte representation.
  (qa (aref (substring string 126 130) 2) 128)
  (qa (vconcat (substring string 126 130)) [126 127 128 129])
  (qa (vconcat (concat string "x")) (apply #'vector (append bytes '(120))))
  (qa (multibyte-string-p string) nil)
  (qa (length string) 256))
(qa (vconcat (string-as-unibyte "éあ")) [195 169 227 129 130])
(qa (vconcat (make-string 3 255 nil)) [255 255 255])
;; `propertize' preserves the source's explicit unibyte representation.
(qa (aref (propertize (unibyte-string 128 255) 'face 'bold) 0) 128)
(qa (vconcat (propertize (unibyte-string 128 255) 'face 'bold)) [128 255])

;; A multibyte string's actual eight-bit characters must stay in their
;; character-code domain; Unicode characters must not be truncated to bytes.
(qa (vconcat (string-to-multibyte (unibyte-string 128 255))) [4194176 4194303])
(qa (vconcat "éあ") [233 12354])
(qa (vconcat "\u0080x") [128 120])
(qa (vconcat (string 0 127 128 255 256 12354)) [0 127 128 255 256 12354])

;; Sharing the sequence converter must preserve other sequence behavior.
(qa (vconcat) [])
(qa (vconcat nil "" [] (bool-vector)) [])
(qa (vconcat '(a 5) [nil z] (bool-vector t nil) (unibyte-string 128))
    [a 5 nil z t nil 128])
(let* ((source [a b]) (result (vconcat source)))
  (qa (eq source result) nil)
  (aset result 0 'changed)
  (qa source [a b]))
(let ((element (list 'shared)))
  (qa (eq (aref (vconcat (list element)) 0) element) t))
(let ((cycle (list 1)))
  (setcdr cycle cycle)
  (qa-err (vconcat cycle) 'circular-list))
(qa-err (vconcat 5) 'wrong-type-argument)
(qa-err (vconcat (record 'item 1)) 'wrong-type-argument)
(qa-err (vconcat (make-char-table 'general)) 'wrong-type-argument)
(qa-err (vconcat '(1 . 2)) 'wrong-type-argument)
(princ (qa--done)) (terpri)
