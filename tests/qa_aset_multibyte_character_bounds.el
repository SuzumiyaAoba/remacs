;;; -*- lexical-binding: nil; -*-
;; `aset' validates character values before applying string-storage rules (#593).
(qa
 (condition-case err
     (progn (aset (copy-sequence "éé") 0 233) 'stored)
   (error (error-message-string err)))
 "Attempt to store non-ASCII char into multibyte string")
(qa
 (condition-case err
     (progn (aset (string-as-multibyte "abc") 0 4294967361) 'stored)
   (error (list (car err) (cdr err))))
 '(wrong-type-argument (characterp 4294967361)))
(qa
 (condition-case err
     (progn (aset (string-as-unibyte "a") 0 #x110000) 'stored)
   (error (error-message-string err)))
 "Attempt to store non-byte value into unibyte string")
(qa
 (condition-case err
     (progn (aset (string-as-multibyte "a") 0 #x110000) 'stored)
   (error (error-message-string err)))
 "Attempt to store non-ASCII char into multibyte string")
(qa
 (let ((s (copy-sequence "abc")))
   (list (aset s 0 ?Z) s))
 '(90 "Zbc"))
(princ (qa--done))
