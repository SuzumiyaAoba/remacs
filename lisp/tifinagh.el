;;; tifinagh.el --- Quail package for inputting Tifinagh	-*- coding: utf-8; lexical-binding:t -*-

;; Copyright (C) 2024-2026 Free Software Foundation, Inc.

;; Author: Adam Oudad <adam.oudad@gmail.com>
;; Keywords: mule, input method, Tifinagh

(require 'quail)

(quail-define-package
 "tifinagh" "Tininagh" "ⵣ" nil "Tifinagh input method.

Based on Tifinagh table in X Keyboard Configuration DB.
To enter any assigned character in the Unicode Tifinagh block, type a
semicolon followed by its four-digit code point in hexadecimal, such as ;2d66.
" nil t t t t nil nil nil nil nil t)

;; The XKB-derived table is supplemented below with a Unicode code-point
;; fallback for every assigned character in the Tifinagh block.
(quail-define-rules
 ("Q" ?ⵈ)
 ("W" ?ⵯ)
 ("R" ?ⵕ)
 ("T" ?ⵟ)
 ("P" ?ⵒ)

 ("S" ?ⵚ)
 ("D" ?ⴹ)
 ("G" ?ⴶ)
 ("H" ?ⵂ)
 ("J" ?ⵌ)
 ("K" ?ⴾ)

 ("Z" ?ⵥ)
 ("X" ?ⵝ)
 ("C" ?ⵞ)
 ("V" ?ⵗ)

 ("q" ?ⵇ)
 ("w" ?ⵡ)
 ("e" ?ⴻ)
 ("r" ?ⵔ)
 ("t" ?ⵜ)
 ("y" ?ⵢ)
 ("u" ?ⵓ)
 ("i" ?ⵉ)
 ("o" ?ⵄ)
 ("p" ?ⵃ)

 ("a" ?ⴰ)
 ("s" ?ⵙ)
 ("d" ?ⴷ)
 ("f" ?ⴼ)
 ("g" ?ⴳ)
 ("h" ?ⵀ)
 ("j" ?ⵊ)
 ("k" ?ⴽ)
 ("l" ?ⵍ)

 ("z" ?ⵣ)
 ("x" ?ⵅ)
 ("c" ?ⵛ)
 ("v" ?ⵖ)
 ("b" ?ⴱ)
 ("n" ?ⵏ)
 ("m" ?ⵎ)
 )

;; Keep a code-point entry path for every assigned character in the block,
;; including characters that have no key in the XKB-derived table.
(dolist (codepoint
         (append (number-sequence #x2d30 #x2d67)
                 '(#x2d6f #x2d70 #x2d7f)))
  (let ((character (decode-char 'ucs codepoint)))
    (when character
      (quail-defrule (format ";%04x" codepoint) character)
      (quail-defrule (format ";%04X" codepoint) character))))

;;; tifinagh.el ends here
