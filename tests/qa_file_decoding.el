;;; -*- lexical-binding: t; -*-
;; Issue #156. tests/file_coding.rs creates exact binary fixtures and supplies
;; qa-file-coding-dir. The same forms are checked against GNU Emacs 31.1.
(defvar qa-file-coding-dir)
(defun qa-file-read (name coding multi &optional beg end literal)
  (with-temp-buffer
    (set-buffer-multibyte multi)
    (let* ((coding-system-for-read coding)
           (file (expand-file-name name qa-file-coding-dir))
           (ret (if literal
                    (insert-file-contents-literally file nil beg end)
                  (insert-file-contents file nil beg end))))
      (list (cadr ret) (string-to-list (buffer-string))
            last-coding-system-used (get-text-property 1 'charset)))))

;; Counts are decoded characters, while BEG/END select file bytes.
(qa (qa-file-read "valid" nil t) '(1 (233) utf-8 nil))
(qa (qa-file-read "mixed" 'utf-8 t) '(2 (233 4194303) utf-8 nil))
(qa (qa-file-read "mixed" nil t) '(2 (233 4194303) utf-8 nil))
(qa (qa-file-read "original" nil t)
    '(15 (108 105 110 101 49 10 108 105 110 101 50 195 169 255 10)
         iso-latin-1-unix iso-8859-1))
(qa (qa-file-read "original" 'utf-8 t)
    '(14 (108 105 110 101 49 10 108 105 110 101 50 233 4194303 10)
         utf-8-unix nil))
(qa (qa-file-read "original" 'raw-text t)
    '(15 (108 105 110 101 49 10 108 105 110 101 50 4194243 4194217 4194303 10)
         raw-text-unix nil))
(qa (qa-file-read "mixed" 'binary t)
    '(3 (4194243 4194217 4194303) binary nil))
(qa (qa-file-read "mixed" 'no-conversion t)
    '(3 (4194243 4194217 4194303) no-conversion nil))
(qa (qa-file-read "mixed" 'raw-text t)
    '(3 (4194243 4194217 4194303) raw-text nil))
(qa (qa-file-read "mixed" 'utf-8 t 0 1) '(1 (4194243) utf-8 nil))
(qa (qa-file-read "mixed" 'utf-8 t 1 2) '(1 (4194217) utf-8 nil))
(qa (qa-file-read "mixed" 'utf-8 t 0 2) '(1 (233) utf-8 nil))
(qa (qa-file-read "invalid" 'utf-8 t)
    '(4 (4194303 4194176 4194240 4194223) utf-8 nil))
(qa (qa-file-read "truncated" 'utf-8 t)
    '(2 (4194275 4194177) utf-8 nil))

(qa (qa-file-read "utf8-boundaries" 'utf-8 t)
    '(11 (4194285 4194208 4194176 4194288 4194176 4194176 4194223
              4194274 40 4194209 1114111) utf-8 nil))

;; Explicit encodings and aliases must affect the decoder, not only metadata.
(qa (qa-file-read "valid" 'iso-latin-1 t)
    '(2 (195 169) iso-latin-1 iso-8859-1))
(qa (qa-file-read "latin" 'iso-8859-1 t)
    '(1 (233) iso-8859-1 iso-8859-1))
(qa (qa-file-read "valid" 'cp65001 t) '(1 (233) cp65001 nil))
(qa (qa-file-read "shift-jis" 'japanese-shift-jis t)
    '(1 (12354) japanese-shift-jis japanese-jisx0208))

;; A BOM is stripped only when signature detection/decoding requests it.
(qa (qa-file-read "bom" nil t) '(1 (233) utf-8-with-signature nil))
(qa (qa-file-read "bom" 'utf-8 t) '(2 (65279 233) utf-8 nil))
(qa (qa-file-read "bom" 'utf-8-with-signature t)
    '(1 (233) utf-8-with-signature nil))
(qa (qa-file-read "bom" 'raw-text t)
    '(5 (4194287 4194235 4194239 4194243 4194217) raw-text nil))
(qa (qa-file-read "valid" 'utf-8 t nil nil t)
    '(2 (4194243 4194217) no-conversion nil))

;; EOL decoding is independent of character conversion.
(qa (qa-file-read "crlf" nil t) '(4 (65 10 66 10) undecided-dos nil))
(qa (qa-file-read "crlf" 'raw-text t) '(4 (65 10 66 10) raw-text-dos nil))
(qa (qa-file-read "crlf" 'utf-8 t) '(4 (65 10 66 10) utf-8-dos nil))
(qa (qa-file-read "crlf" 'binary t) '(6 (65 13 10 66 13 10) binary nil))
(qa (qa-file-read "crlf" 'utf-8-unix t)
    '(6 (65 13 10 66 13 10) utf-8-unix nil))
(qa (qa-file-read "crlf" 'cp65001-dos t)
    '(4 (65 10 66 10) cp65001-dos nil))
(qa (qa-file-read "mac" 'raw-text t) '(4 (65 10 66 10) raw-text-mac nil))
(qa (qa-file-read "mixed-eol" 'utf-8 t)
    '(5 (65 13 10 66 10) utf-8-unix nil))

;; Unibyte buffers suppress character decoding, preserving EOL handling.
(qa (qa-file-read "mixed" nil nil) '(3 (195 169 255) raw-text nil))
(qa (qa-file-read "mixed" 'utf-8 nil) '(3 (195 169 255) raw-text nil))
(qa (qa-file-read "mixed" 'iso-latin-1 nil) '(3 (195 169 255) raw-text nil))
(qa (qa-file-read "mixed" 'binary nil) '(3 (195 169 255) binary nil))
(qa (qa-file-read "crlf" 'utf-8 nil) '(4 (65 10 66 10) raw-text-dos nil))
(qa (qa-file-read "bom" nil nil) '(5 (239 187 191 195 169) raw-text nil))

;; Charset properties are placed only on the inserted range.
(qa (with-temp-buffer
      (insert "xx") (goto-char 2)
      (let ((coding-system-for-read 'iso-latin-1))
        (insert-file-contents (expand-file-name "latin" qa-file-coding-dir)))
      (list (string-to-list (buffer-string)) (point)
            (get-text-property 1 'charset) (get-text-property 2 'charset)
            (get-text-property 3 'charset)))
    '((120 233 120) 2 nil iso-8859-1 nil))
(qa (with-temp-buffer
      (insert "old contents")
      (goto-char (point-min))
      (let ((coding-system-for-read 'utf-8))
        (let ((ret (insert-file-contents
                    (expand-file-name "valid" qa-file-coding-dir) nil nil nil t)))
          (list (cadr ret) (string-to-list (buffer-string)) (point)))))
    '(1 (233) 1))
(qa-err-data (qa-file-read "valid" 'qa-no-such-coding t)
             'coding-system-error '(qa-no-such-coding))

;; Invalid bytes survive subsequent encoding instead of becoming U+FFFD.
(qa (with-temp-buffer
      (let ((coding-system-for-read 'utf-8))
        (insert-file-contents (expand-file-name "mixed" qa-file-coding-dir)))
      (string-to-list (encode-coding-string (buffer-string) 'utf-8)))
    '(195 169 255))
(qa (with-temp-buffer
      (let ((coding-system-for-read 'raw-text))
        (insert-file-contents (expand-file-name "mixed" qa-file-coding-dir)))
      (string-to-list (encode-coding-string (buffer-string) 'raw-text)))
    '(195 169 255))

;; Detection respects coding-system priorities; a signature still wins.
(let ((priorities (coding-system-priority-list)))
  (unwind-protect
      (progn
        (prefer-coding-system 'iso-latin-1)
        (qa (qa-file-read "valid" nil t)
            '(2 (195 169) iso-latin-1 iso-8859-1))
        (qa (qa-file-read "bom" nil t)
            '(1 (233) utf-8-with-signature nil)))
    (apply #'set-coding-system-priority priorities)))
(princ (qa--done))
