;;; -*- lexical-binding: nil; -*-
;; `string' promotes to multibyte only for non-ASCII character codes (#554).
(qa (mapcar #'multibyte-string-p
            (list (string) (string 65) (string 128) (string #x3fff80)))
    '(nil nil t t))
(qa (list (string-to-list (string 65))
          (string-to-list (string 128))
          (multibyte-string-p (string 65)))
    '((65) (128) nil))
(qa-err (string 'not-a-character) 'wrong-type-argument)
(princ (qa--done))
