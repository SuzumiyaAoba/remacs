;;; -*- lexical-binding: nil; -*-
;; #624: character iteration must retain the actual Emacs character code.
(dolist (case '((#xf0080 unicode)
                (#xf0100 unicode)
                (#xd800 unicode-bmp)
                (#x110000 chinese-gb2312)
                (#x200000 gb18030)
                (#x3fff80 eight-bit)
                (#x3fffff eight-bit)))
  (let ((code (car case)) (charset (cadr case)))
    (qa (char-charset code) charset)
    (qa (find-charset-string (string code)) (list charset))
    (qa (with-temp-buffer
          (insert (string code))
          (find-charset-region (point-min) (point-max)))
        (list charset))))
(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (insert (unibyte-string 128 255 65))
      (list (find-charset-region (point-min) (point-max))
            (find-charset-string (buffer-string))
            (string-to-list (buffer-string)) (point)))
    '((ascii eight-bit) (ascii eight-bit) (128 255 65) 4))
(qa (with-temp-buffer
      (insert (string 128 255 65))
      (list (find-charset-region (point-min) (point-max))
            (find-charset-string (buffer-string))))
    '((ascii unicode-bmp) (ascii unicode-bmp)))
(princ (qa--done))
