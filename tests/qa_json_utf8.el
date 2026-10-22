;;; qa_json_utf8.el --- Byte-oriented JSON string decoding -*- lexical-binding: t; -*-
;; GNU 31.1 reference: literals decode strictly; syntax bytes outside strings
;; retain their parse/trailing-content errors. Positions count source units.

;; valid U+0080
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 194 128 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((128) t))

;; valid U+07FF
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 223 191 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((2047) t))

;; valid U+0800
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 224 160 128 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((2048) t))

;; valid U+D7FF
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 237 159 191 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((55295) t))

;; valid U+E000
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 238 128 128 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((57344) t))

;; valid U+FFFF
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 239 191 191 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((65535) t))

;; valid U+10000
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 240 144 128 128 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((65536) t))

;; valid U+10FFFF
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 34 244 143 191 191 34)))) (list (string-to-list value) (multibyte-string-p value))) (error error))
    '((1114111) t))

;; mixed literal and escape
(qa (condition-case error (string-to-list (json-parse-string (unibyte-string 34 195 169 92 117 48 51 66 66 34))) (error error))
    '(233 955))

;; UTF-8 object key alist
(qa (condition-case error (let ((entry (car (json-parse-string (unibyte-string 123 34 195 169 34 58 49 125) :object-type 'alist)))) (list (string-to-list (symbol-name (car entry))) (cdr entry))) (error error))
    '((233) 1))

;; UTF-8 object key plist
(qa (condition-case error (let ((value (json-parse-string (unibyte-string 123 34 195 169 34 58 49 125) :object-type 'plist))) (list (string-to-list (symbol-name (car value))) (cadr value))) (error error))
    '((58 233) 1))

;; multibyte source
(qa (condition-case error (string-to-list (json-parse-string "\"é日本😀\"")) (error error))
    '(233 26085 26412 128512))

;; invalid FF
(qa (condition-case error (json-parse-string (unibyte-string 34 255 34)) (error error))
    '(json-utf8-decode-error 1 nil 3))

;; lone continuation
(qa (condition-case error (json-parse-string (unibyte-string 34 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 3))

;; invalid second byte
(qa (condition-case error (json-parse-string (unibyte-string 34 195 34)) (error error))
    '(json-utf8-decode-error 1 nil 3))

;; overlong two-byte
(qa (condition-case error (json-parse-string (unibyte-string 34 192 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 3))

;; overlong three-byte
(qa (condition-case error (json-parse-string (unibyte-string 34 224 128 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 4))

;; overlong four-byte
(qa (condition-case error (json-parse-string (unibyte-string 34 240 128 128 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 5))

;; surrogate
(qa (condition-case error (json-parse-string (unibyte-string 34 237 160 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 4))

;; above Unicode
(qa (condition-case error (json-parse-string (unibyte-string 34 244 144 128 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 5))

;; invalid long lead
(qa (condition-case error (json-parse-string (unibyte-string 34 248 128 34)) (error error))
    '(json-utf8-decode-error 1 nil 3))

;; missing second byte
(qa (condition-case error (json-parse-string (unibyte-string 34 195)) (error error))
    '(json-end-of-file 1 nil 2))

;; missing third byte
(qa (condition-case error (json-parse-string (unibyte-string 34 226 130)) (error error))
    '(json-end-of-file 1 nil 3))

;; missing fourth byte
(qa (condition-case error (json-parse-string (unibyte-string 34 240 144 128)) (error error))
    '(json-end-of-file 1 nil 4))

;; missing fourth after bad third
(qa (condition-case error (json-parse-string (unibyte-string 34 240 144 34)) (error error))
    '(json-end-of-file 1 nil 4))

;; invalid root
(qa (condition-case error (json-parse-string (unibyte-string 255)) (error error))
    '(json-parse-error 1 nil 1))

;; invalid trailing byte
(qa (condition-case error (json-parse-string (unibyte-string 49 32 255)) (error error))
    '(json-trailing-content 1 nil 3))

;; invalid continuation newline
(qa (condition-case error (json-parse-string (unibyte-string 34 195 10 34)) (error error))
    '(json-utf8-decode-error 2 nil 3))

;; invalid key
(qa (condition-case error (json-parse-string (unibyte-string 123 34 255 34 58 49 125)) (error error))
    '(json-utf8-decode-error 1 nil 4))

;; multibyte raw-byte character
(qa (condition-case error (json-parse-string (concat "\"λ" (unibyte-string 255) "\"")) (error error))
    '(json-utf8-decode-error 1 nil 3))

;; UTF-8 error hierarchy
(qa (condition-case error (condition-case nil (json-parse-string (unibyte-string 34 255 34)) (json-parse-error 'wrong-parent) (json-error 'json-error)) (error error))
    'json-error)

;; unibyte buffer
(qa (condition-case error (with-temp-buffer (set-buffer-multibyte nil) (insert (unibyte-string 34 195 169 34)) (goto-char 1) (let ((value (json-parse-buffer))) (list (string-to-list value) (multibyte-string-p value) (point)))) (error error))
    '((233) t 5))

;; invalid unibyte buffer
(qa (condition-case error (with-temp-buffer (set-buffer-multibyte nil) (insert (unibyte-string 34 255 34)) (goto-char 1) (list (condition-case error (json-parse-buffer) (error error)) (point))) (error error))
    '((json-utf8-decode-error 1 nil 3) 1))

(princ (qa--done))
