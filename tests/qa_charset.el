;; Charset / ucs / char utilities — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - charsetp works on unicode/iso-8859-1.
;;   - charsetp-list is NOT a fn.
;;   - charset-after returns the charset of the char at point.
;;   - find-charset-string returns a charset list for the string's chars.
;;   - ucs-normalize-* (NFC/NFD/NFKC/NFKD) are not in 31.1 batch — they
;;     live in ucs-normalize.el which is not preloaded.
;;   - ucs-normalize-string is bound (it's in the dump).
;;   - encode-char/decode-char roundtrip between charsets.
;;   - char-width/string-width/truncate-string-to-width use char-width-table.
;;   - charset-id-internal / charset-ordinal-pos are NOT fns.
;;   - charset-map / clear-charset-maps are NOT fns.

;; == charsetp / charset-after ==
(qa (charsetp 'unicode) t)
(qa (charsetp 'iso-8859-1) t)
(qa (charsetp 'no-such) nil)
(qa (fboundp 'charsetp-list) nil)
(with-temp-buffer
  (insert "abc")
  (goto-char 1)
  (qa (charset-after) 'ascii))

;; == charset fn surface ==
(qa (fboundp 'find-charset-region) t)
(qa (fboundp 'charset-id) nil)
(qa (fboundp 'charset-ordinal-pos) nil)
(qa (fboundp 'charset-id-internal) t)
(qa (fboundp 'charset-plist) t)
(qa (fboundp 'charset-priority-list) t)
(qa (fboundp 'set-charset-priority) t)
(qa (fboundp 'charset-chars) t)
(qa (fboundp 'charset-dimension) t)
(qa (fboundp 'charset-offset) nil)
(qa (fboundp 'map-charset-chars) t)
(qa (fboundp 'clear-charset-maps) t)
(qa (fboundp 'charset-map) nil)
(qa (fboundp 'load-charset) nil)
(qa (fboundp 'charset-from-name) nil)
;; == find-charset-string ==
(qa (equal (find-charset-string "abc") '(ascii)) t)
;; GNU's default charset priority selects Unicode for these characters.
(qa (find-charset-string "abc 日本語") '(ascii unicode-bmp))

;; == char-width / string-width ==
(qa (char-width ?a) 1)
(qa (char-width ?日) 2)
(qa (string-width "abc") 3)
(qa (string-width "日本") 4)
(qa (truncate-string-to-width "abcdef" 4) "abcd")
(qa (truncate-string-to-width "abc" 10) "abc")
;; string-width with ignored newlines

;; newlines don't count toward width
(qa (string-width "a\nb") 2)
(qa (fboundp 'ucs-normalize-NFC) nil)
(qa (fboundp 'ucs-normalize-NFD) nil)
(qa (fboundp 'ucs-normalize-NFKC) nil)
(qa (fboundp 'ucs-normalize-NFKD) nil)
(qa (fboundp 'ucs-normalize-string) t)

;; == encode/decode-char ==
(qa (encode-char ?a 'ascii) 97)
(qa (encode-char ?a 'unicode) 97)
(qa (decode-char 'ascii 97) 97)
(qa (decode-char 'unicode 97) 97)
;; out-of-range → nil
(qa (decode-char 'ascii 256) nil)

;; == char-to-string / string-to-char ==
(qa (char-to-string ?a) "a")
(qa (char-to-string ?日) "日")
(qa (string-to-char "a") 97)
(qa (string-to-char "abc") 97)

;; == charset-plist returns a plist ==
(qa (listp (charset-plist 'unicode)) t)

;; == charset-priority-list is a list ==
(qa (listp (charset-priority-list)) t)
(qa (if (memq 'unicode (charset-priority-list)) t nil) t)

;; == GNU charset.c semantics (31.1, locale-independent forms) ==
;; ASCII chars are hardwired to charset `ascii' (CHAR_CHARSET macro):
;; they never reach the ordered-list walk, so even higher-priority
;; JIS/Latin charsets don't claim them.
(qa (char-charset 97) 'ascii)
(qa (char-charset 0) 'ascii)
(qa (char-charset 9) 'ascii)
;; …but a restrictive list does see them.
(qa (char-charset 97 '(latin-jisx0201 ascii)) 'latin-jisx0201)
(qa (char-charset 97 '(ascii latin-jisx0201)) 'ascii)
;; Coding-system restriction uses its :charset-list.
(qa (char-charset 97 'iso-8859-1) 'iso-8859-1)
(qa (char-charset 12354 'iso-2022-jp) 'japanese-jisx0208)
;; Internal codes above Unicode: gb2312's :code-offset range claims
;; #x110000; emacs covers through MAX_5_BYTE_CHAR; eight-bit above.
(qa (char-charset #x110000) 'chinese-gb2312)
(qa (char-charset #x3FFF7F) 'emacs)
(qa (char-charset #x3FFF80) 'eight-bit)
(qa (char-charset #x3FFFFF) 'eight-bit)
;; out-of-range → wrong-type-argument characterp
(qa (condition-case e (char-charset -1) (error (car e))) 'wrong-type-argument)
(qa (condition-case e (char-charset #x400000) (error (car e))) 'wrong-type-argument)

;; == decode-char ==
(qa (decode-char 'unicode 65.0) 65)          ; integral float accepted
(qa (condition-case e (decode-char 'unicode -1) (error (car e))) 'error)
(qa (decode-char 'japanese-jisx0208 '(36 34)) nil)  ; cons packing, unmapped
(qa (decode-char 'emacs #x3FFF80) nil)       ; emacs max = MAX_5_BYTE_CHAR
(qa (decode-char 'eight-bit #x80) 4194176)
(qa (decode-char 'eight-bit #x7F) nil)
;; non-integer arg → error "Not an in-range…"
(qa (condition-case e (decode-char 'unicode "65") (error (car e))) 'error)
(qa (condition-case e (decode-char 'japanese-jisx0208 '(36)) (error (car e))) 'error)

;; == encode-char ==
(qa (encode-char #x3FFF80 'eight-bit) #x80)
(qa (encode-char #x3FFF7F 'eight-bit) nil)
(qa (encode-char #x3FFF80 'emacs) nil)       ; eight-bit not in emacs
(qa (encode-char #x110000 'unicode) nil)
(qa (condition-case e (encode-char 97 'bogus) (error (car e))) 'wrong-type-argument)

;; == make-char ==
(qa (make-char 'ascii) 0)
(qa (make-char 'ascii 0) 0)
(qa (make-char 'ascii 255) 127)              ; iso-final masks to 7 bits
(qa (make-char 'ascii 128) 0)
(qa (make-char 'unicode 1 246 0) #x1F600)
(qa (make-char 'unicode 16 255 255) #x10FFFF)
(qa (make-char 'katakana-jisx0201 65) 65409) ; halfwidth katakana A
(qa (make-char 'japanese-jisx0208 36 34) 12354)
(qa (condition-case e (make-char 'unicode 65) (error (car e))) 'error)
(qa (condition-case e (make-char 'unicode 17 0 0) (error (car e))) 'error)
(qa (condition-case e (make-char 'iso-8859-1 256) (error (car e))) 'args-out-of-range)
(qa (condition-case e (make-char 'ascii -1) (error (car e))) 'wrong-type-argument)
(qa (condition-case e (make-char 'bogus 65) (error (car e))) 'wrong-type-argument)

;; == split-char ==
(qa (split-char 97) '(ascii 97))
(qa (split-char #x3FFF80) '(eight-bit 128))
(qa (split-char #x3FFFFF) '(eight-bit 255))
(qa (split-char #x10FFFF) '(unicode 16 255 255))
(qa (split-char #x110000) '(chinese-gb2312 33 33))
(qa (condition-case e (split-char -1) (error (car e))) 'wrong-type-argument)

;; == char-width (GNU default char-width-table) ==
(qa (char-width 9) 8)                        ; TAB = tab-width
(qa (char-width 10) 0)                       ; newline width 0
(qa (char-width #x80) 4)                     ; ISO control
(qa (char-width #x9F) 4)
(qa (char-width #xD800) 1)                   ; lone surrogate
(qa (char-width #x3FFF80) 4)                 ; eight-bit
(qa (char-width #x3FFFFF) 4)
(qa (char-width #x110000) 1)                 ; private mule char
(qa (condition-case e (char-width -1) (error (car e))) 'wrong-type-argument)
(qa (condition-case e (char-width #x400000) (error (car e))) 'wrong-type-argument)

;; == string-width bounds ==
(qa (string-width "abcdef" -1) 1)            ; negative counts from end
(qa (condition-case e (string-width "abcdef" 0 99) (error (car e))) 'args-out-of-range)
(qa (condition-case e (string-width "abcdef" 9) (error (car e))) 'args-out-of-range)
(qa (condition-case e (string-width nil) (error (car e))) 'wrong-type-argument)

;; == charset attribute plists (GNU synthesized values) ==
(qa (get-charset-property 'unicode :dimension) 3)
(qa (equal (get-charset-property 'unicode :code-space) [0 255 0 255 0 16 0 0]) t)
(qa (get-charset-property 'eight-bit :code-offset) 4194176)
(qa (get-charset-property 'ascii :iso-final-char) 66)
(qa (get-charset-property 'ascii :emacs-mule-id) 0)
(qa (get-charset-property 'emacs :dimension) 3)
(qa (condition-case e (charset-plist 'bogus) (error (car e))) 'wrong-type-argument)

(princ (qa--done))
