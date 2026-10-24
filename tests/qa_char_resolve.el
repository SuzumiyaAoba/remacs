;;; -*- lexical-binding: t; -*-
;; Reference values and complete ASCII/modifier matrix from GNU Emacs 31.1.
(qa (mapcar #'char-resolve-modifiers
            '(97 33554529 67108961 67108929 100663393
              33554481 67108896 67108927 67108873 134217825))
    '(97 65 1 1 1 33554481 0 127 67108873 134217825))
(qa (char-resolve-modifiers -1) -1)
(qa (char-resolve-modifiers most-negative-fixnum) most-negative-fixnum)
(qa (char-resolve-modifiers most-positive-fixnum) most-positive-fixnum)
(qa (char-resolve-modifiers (logior 233 #x6000000)) (logior 233 #x6000000))
(qa (char-resolve-modifiers (logior (ash 1 32) #x2000000 97))
    (logior (ash 1 32) #x2000000 97))
(qa-err-data (char-resolve-modifiers nil) 'wrong-type-argument '(fixnump nil))
(qa-err-data (char-resolve-modifiers t) 'wrong-type-argument '(fixnump t))
(qa-err-data (char-resolve-modifiers 1.0) 'wrong-type-argument '(fixnump 1.0))
(qa-err (char-resolve-modifiers (1+ most-positive-fixnum)) 'wrong-type-argument)
(qa (let (values)
      (dotimes (mask 64)
        (dotimes (character 128)
          (push (char-resolve-modifiers (logior character (ash mask 22))) values)))
      (list (length values) (md5 (prin1-to-string (nreverse values)))))
    '(8192 "da3c1e8f778dd359f3efa96c150171c1"))
(princ (qa--done)) (terpri)
