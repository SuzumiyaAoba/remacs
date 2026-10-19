;;; -*- lexical-binding: nil; -*-
;; Issue #26: verify both the character and the string representation.
;; All expectations are checked by GNU Emacs 31.1 as well as remacs.
(defun qa26-observe (character)
  (let ((description (text-char-description character)))
    (list (string-to-list description) (length description)
          (multibyte-string-p description) (string-bytes description))))

;; ASCII caret descriptions are unibyte, unlike non-ASCII descriptions.
(dolist (qa26-char (number-sequence 0 31))
  (qa (qa26-observe qa26-char) (list (list 94 (+ 64 qa26-char)) 2 nil 2)))
(dolist (qa26-char (number-sequence 32 126))
  (qa (qa26-observe qa26-char) (list (list qa26-char) 1 nil 1)))
(qa (qa26-observe 127) '((94 63) 2 nil 2))

;; C1 controls are actual character codes, not four-character octal text.
(dolist (qa26-char (number-sequence 128 255))
  (qa (qa26-observe qa26-char) (list (list qa26-char) 1 t 2)))

;; Eight-bit characters remain eight-bit codes in a multibyte string;
;; they use the two-byte internal BYTE8 representation, not '?'.
(dolist (qa26-char (number-sequence #x3fff80 #x3fffff))
  (qa (qa26-observe qa26-char) (list (list qa26-char) 1 t 2)))

;; Rust char cannot represent surrogates or Emacs' extended Mule range.
(dolist (qa26-case '((#xd7ff . 3) (#xd800 . 3) (#xdfff . 3) (#xe000 . 3)
                     (#xffff . 3) (#x10000 . 4) (#x10ffff . 4)
                     (#x110000 . 4) (#x1fffff . 4) (#x200000 . 5) (#x3fff7f . 5)))
  (qa (qa26-observe (car qa26-case))
      (list (list (car qa26-case)) 1 t (cdr qa26-case)))
  (qa (string-bytes (string (car qa26-case))) (cdr qa26-case)))

(qa (text-char-description 4194176) (string (decode-char 'eight-bit 128)))
(qa (text-char-description 4194303) (string (decode-char 'eight-bit 255)))
(qa (text-char-description 128) (string 128))
(dolist (qa26-invalid '(-1 #x400000 #x8000000 2305843009213693952 nil t name 1.5 "a" []))
  (qa (condition-case err (text-char-description qa26-invalid) (error err))
      (list 'wrong-type-argument 'characterp qa26-invalid)))
(qa-err (text-char-description) 'wrong-number-of-arguments)
(qa-err (text-char-description 65 t) 'wrong-number-of-arguments)

;; The accompanying unibyte-char-to-multibyte report was already fixed.
(qa-err-data (unibyte-char-to-multibyte 4194176)
             'error '("Not a unibyte character: 4194176"))
(qa-err-data (unibyte-char-to-multibyte 4194303)
             'error '("Not a unibyte character: 4194303"))

(princ (qa--done)) (terpri)
