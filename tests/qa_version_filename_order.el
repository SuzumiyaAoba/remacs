;;; -*- lexical-binding: nil; -*-
;; GNU 31.1 filename-version ordering (#412, #413).
(let* ((ordered '("" "." ".." ".~" ".0" ".a" ".a.tar" ".a.1" ".b"
                  "~" "a" "a.tar.gz" "a1~" "a1~rc1" "a1" "a1.tar"
                  "a1.tar.gz" "a1a" "a1+" "a1.0" "a1.0.tar" "a1.1"
                  "a1.1.tar" "a2" "a10" "a-1" "a.1.tar.gz" "a.2.tar.gz"
                  "a.10.tar.gz" "a_1" "z"))
       (left 0))
  (dolist (a ordered)
    (let ((right 0))
      (dolist (b ordered)
        (qa (string-version-lessp a b) (< left right))
        (setq right (1+ right))))
    (setq left (1+ left))))
(qa (string-version-lessp "a" "a0") nil)
(qa (string-version-lessp "a0" "a") nil)
(qa (string-version-lessp "a1" "a01") nil)
(qa (string-version-lessp "a01" "a1") nil)
(qa (string-version-lessp nil "x") t)
(qa (string-version-lessp "x" nil) nil)
(qa (string-version-lessp nil 'nil) nil)
(qa (string-version-lessp 'a 'b) t)
;; GNU compares storage bytes, not Unicode codepoint order.
(qa (string-version-lessp (unibyte-string 255) (string 256)) nil)
(qa (string-version-lessp (string 256) (unibyte-string 255)) t)
(qa (string-version-lessp (make-symbol (unibyte-string 255)) (string 256)) nil)
(qa (string-version-lessp (string 256) (make-symbol (unibyte-string 255))) t)
(qa (string-version-lessp "x\0a" "x\0b") t)
(qa (string-version-lessp "x\0b" "x\0a") nil)
(qa-err-data (string-version-lessp 42 "x") 'wrong-type-argument '(stringp 42))
(qa-err-data (string-version-lessp "x" 42) 'wrong-type-argument '(stringp 42))
(princ (qa--done))
