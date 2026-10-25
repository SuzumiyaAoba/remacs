;;; -*- lexical-binding: nil; -*-
;; string-as-multibyte decodes canonical Emacs UTF-8 including Mule and surrogates (#427).
(qa (string-to-list
     (string-as-multibyte (unibyte-string #xf8 #x88 #x80 #x80 #x80)))
    '(#x200000))
(qa (string-to-list
     (string-as-multibyte (unibyte-string #xed #xa0 #x80)))
    '(#xd800))
(qa (string-to-list
     (string-as-multibyte (unibyte-string #xf0 #x80 #x80 #x80)))
    '(#x3ffff0 #x3fff80 #x3fff80 #x3fff80))
(princ (qa--done))
