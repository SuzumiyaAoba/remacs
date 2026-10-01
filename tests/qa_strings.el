;; strings — comparisons, trims/splits, width, substrings, case, multibyte,
;; coding-system roundtrips, string<->number, format, hashing.
;; GNU-verified quirks pinned below:
;;  - string= coerces symbols via symbol-name; nil = the string "nil".
;;  - string-blank-p returns the match index 0, not t; it errors on nil while
;;    string-empty-p returns nil for any non-string.
;;  - case-fold-search (t under -Q --batch) makes string-match-p case-insensitive.
;;  - concat UTF-8-decodes unibyte byte args, but string-to-multibyte keeps
;;    each byte as an eight-bit char (byte-preserving, not decoding).
;;  - upcase/downcase/capitalize accept chars; non-string non-chars are
;;    returned unchanged by downcase/capitalize.
;;  - string< string> exist.
;;  - string<= string>= string/= and string-slice do NOT exist (31.1).
;;    string-reverse lives in subr-x: void until an autoload (e.g. the
;;    string-lines / take calls below) pulls the library in.  Order in
;;    this file therefore matters for the void-function check on it.
;;    string-fill REQUIRES WIDTH; string-equal is a separate subr.

(let ((case-fold-search t)) ;; pin default explicitly for match checks below
  (ignore case-fold-search))

;; == equality / ordering ==
(qa (string= "abc" "abc") t)
(qa (string= "abc" "abd") nil)
(qa (string= "Abc" "abc") nil)
(qa (string= nil nil) t)
(qa (string= 'foo "foo") t)
(qa-err (string= "1" 1) 'wrong-type-argument)
(qa-err-data (string= "1" 1) 'wrong-type-argument '(stringp 1))
(qa-err (string= "a" "a" "a") 'wrong-number-of-arguments)
(qa (subrp (symbol-function 'string-equal)) t)
(qa (string-equal "a" "a") t)

(qa (string-equal-ignore-case "AbC" "aBc") t)
(qa (string-equal-ignore-case "abc" "abd") nil)
(qa (string-equal-ignore-case "É" "é") t)

(qa (string< "abc" "abd") t)
(qa (string< "abc" "abc") nil)
(qa (string< "a10" "a9") t)
(qa (string-lessp 'abc "abd") t)
(qa (string> "b" "a") t)
(qa (string> "a" "a") nil)
(qa (string-greaterp "b" "a") t)
(qa-err (string<= "ab" "ab") 'void-function)
(qa-err (string/= "a" "b") 'void-function)
(qa (string-version-lessp "1.2" "1.10") t)
(qa (string-version-lessp "1.2" "1.2") nil)
(qa (string-version-lessp "a1" "a2") t)

;; == compare-strings ==
(qa (compare-strings "abc" nil nil "abc" nil nil) t)
(qa (compare-strings "abc" nil nil "abd" nil nil) -3)
(qa (compare-strings "abd" nil nil "abc" nil nil) 3)
(qa (compare-strings "abc" nil nil "abcdef" nil nil) -4)
(qa (compare-strings "abc" nil nil "xyz" nil nil) -1)
(qa (compare-strings "ABC" nil nil "abc" nil nil) -1)
(qa (compare-strings "ABC" nil nil "abc" nil nil t) t)
(qa (compare-strings "xabc" 1 nil "abc" nil nil) t)
(qa (compare-strings "abcdef" nil nil "xbcde" nil 4) -1)

;; == prefix / suffix / match / search ==
(qa (string-prefix-p "ab" "abc") t)
(qa (string-prefix-p "Ab" "abc") nil)
(qa (string-prefix-p "Ab" "abc" t) t)
(qa (string-prefix-p "" "abc") t)
(qa (string-prefix-p "" "") t)
(qa (string-suffix-p "bc" "abc") t)
(qa (string-suffix-p "BC" "abc") nil)
(qa (string-suffix-p "BC" "abc" t) t)
(qa (string-suffix-p "" "abc") t)

(qa (string-match-p "b+" "abbc") 1)
(qa (string-match-p "B" "abc") 1)
(qa (let ((case-fold-search nil)) (string-match-p "B" "abc")) nil)
(qa (string-match-p "bc" "abcabc" 3) 4)
(qa (progn (string-match "\\(b+\\)" "abbc") (match-beginning 1)) 1)
(qa (progn (string-match "\\(b+\\)" "abbc") (match-end 1)) 3)
(qa-err-data (string-match "x" 5) 'wrong-type-argument '(stringp 5))

(qa (string-search "bc" "abcbc") 1)
(qa (string-search "z" "abc") nil)
(qa (string-search "" "abc") 0)
(qa (string-search "b" "abcb" 2) 3)

;; == blank / empty / trim ==
(qa (string-blank-p "  \t\n") 0)
(qa (string-blank-p " x ") nil)
(qa-err-data (string-blank-p nil) 'wrong-type-argument '(stringp nil))
(qa (string-empty-p "") t)
(qa (string-empty-p "x") nil)
(qa (string-empty-p nil) nil)
(qa (string-empty-p 'a) nil)

(qa (string-trim "  hi  ") "hi")
(qa (string-trim "\n\t hi \n") "hi")
(qa (string-trim "xxhixx" "x+" "x+") "hi")
(qa (string-trim " a" nil "x") "a")
(qa (string-trim-left "  hi  ") "hi  ")
(qa (string-trim-left "xxhi" "x") "xhi")
(qa (string-trim-right "  hi  ") "  hi")
(qa (string-trim-right "hixx" "x+") "hi")

;; == pad / chop / lines / join / split ==
(qa (string-pad "ab" 5) "ab   ")
(qa (string-pad "ab" 5 ?-) "ab---")
(qa (string-pad "ab" 5 ?- t) "---ab")
(qa (string-pad "abcdef" 3) "abcdef")
(qa (string-pad "abc" 3) "abc")

(qa (string-chop-newline "abc\n") "abc")
(qa (string-chop-newline "abc") "abc")
(qa (string-chop-newline "abc\n\n") "abc\n")

(qa (string-lines "a\nb\nc") '("a" "b" "c"))
(qa (string-lines "a\nb\n") '("a" "b"))
(qa (string-lines "a\n\nb" t) '("a" "b"))
(qa (string-lines "") '(""))
(qa (string-lines "a\nb" nil t) '("a\n" "b"))

(qa (string-join '("a" "b" "c") ",") "a,b,c")
(qa (string-join '("a" "b")) "ab")
(qa (string-join nil ",") "")
(qa (string-join ["a" "b"] "-") "a-b")
(qa-err (string-join '(1 2) "-") 'wrong-type-argument)

(qa (string-split "a,b,c" ",") '("a" "b" "c"))
(qa (string-split "a,,b" "," t) '("a" "b"))
(qa (string-split " a , b " "," nil "[ ]") '("a" "b"))
(qa (string-split "a1b22c" "[0-9]+") '("a" "b" "c"))
(qa (string-split " a  b ") '("a" "b"))

;; == limit / width / pixel / truncate / fill ==
(qa (string-limit "abcdef" 3) "abc")
(qa (string-limit "abcdef" 2 t) "ef")
(qa (string-limit "abc" 10) "abc")
(qa-err-data (string-limit "abc" nil) 'wrong-type-argument '(natnump nil))
(qa (string-limit "abé" 3 nil 'utf-8) "ab")
(qa (string-limit "abé" 4 nil 'utf-8) "ab\303\251")

(qa (string-width "abc") 3)
(qa (string-width "日本語") 6)
(qa (string-width "a日b") 4)
(qa (string-width "\t") 8)
(qa (string-width "a\tb") 10)
(qa (string-width "a\nb") 2)
(qa (string-width "é") 3)
(qa (string-pixel-width "abc") 3)
(qa (string-pixel-width "日") 2)

(qa (truncate-string-to-width "日本語" 4) "日本")
(qa (truncate-string-to-width "abcdef" 4 nil nil ?.) "ab…")

(qa (string-fill "aaa bbb ccc" 7) "aaa bbb\nccc")
(qa (string-fill "aaaaaaaaa bbb" 5) "aaaaaaaaa\nbbb")
(qa (string-fill "a b\n\nc d" 10) "a b\n\nc d")
(qa-err (string-fill "abc") 'wrong-number-of-arguments)

;; == replace / clean-whitespace ==
(qa (string-replace "old" "new" "old and old") "new and new")
(qa (string-replace "z" "x" "abc") "abc")
(qa (string-replace "a.b" "X" "a.b axb") "X axb")
(qa (string-replace "OLD" "x" "old OLD") "old x")
(qa (string-replace "\\" "x" "a\\b") "axb")
(qa (string-replace "é" "E" "héllo é") "hEllo E")
(qa-err-data (string-replace "" "x" "abc") 'wrong-length-argument '(0))

(qa (string-clean-whitespace "  a   b \n c  ") "a b c")

;; == substring ==
(qa (substring "abcdef" 1 3) "bc")
(qa (substring "abcdef" -2) "ef")
(qa (substring "abcdef" 1 -1) "bcde")
(qa (substring "abcdef" 2 nil) "cdef")
(qa (substring "abc" 0 0) "")
(qa (substring [a b c] 1) [b c])
(qa-err-data (substring "abc" 5) 'args-out-of-range '("abc" 5 nil))
(qa-err-data (substring "abc" -4) 'args-out-of-range '("abc" -4 nil))
(qa-err-data (substring "abc" 2 1) 'args-out-of-range '("abc" 2 1))
(qa-err-data (substring '(a b c) 1) 'wrong-type-argument '(arrayp (a b c)))
(qa-err-data (substring 5 0) 'wrong-type-argument '(arrayp 5))

(let ((qas-props (propertize "abc" 'face 'bold)))
  (qa (substring qas-props 0 2) "ab")
  (qa (text-properties-at 0 (substring qas-props 0 2)) '(face bold))
  (qa (text-properties-at 0 (substring-no-properties qas-props 0 2)) nil)
  (qa (text-properties-at 0 (copy-sequence qas-props)) '(face bold)))
(qa (substring-no-properties "abc" 1) "bc")
(qa (substring-no-properties "abc" -1) "c")

;; == take / ntake (subr-x, lists only) ==
(qa (take 2 '(a b c d)) '(a b))
(qa (take 0 '(a b)) nil)
(qa (take -1 '(a b c)) nil)
(qa (take 5 '(a b)) '(a b))
(qa-err (take 2 "abcd") 'wrong-type-argument)
(qa-err-data (take 2 [1 2 3]) 'wrong-type-argument '(listp [1 2 3]))
(let ((qas-l (list 1 2 3 4)))
  (qa (ntake 2 qas-l) '(1 2))
  (qa qas-l '(1 2)))
(qa (ntake 5 '(1 2)) '(1 2))

;; == case ==
(qa (upcase "héllo") "HÉLLO")
(qa (upcase "ß") "SS")
(qa (upcase "a1b2") "A1B2")
(qa (upcase ?a) 65)
(qa (downcase "HÉLLO") "héllo")
(qa (downcase ?A) 97)
(qa (downcase 5) 5)
(qa (capitalize "hello WORLD") "Hello World")
(qa (capitalize "éllo") "Éllo")
(qa (capitalize "foo-bar") "Foo-Bar")
(qa (capitalize ?a) 65)
(qa (capitalize 5) 5)
(qa (upcase-initials "hello world") "Hello World")

;; == reverse / concat / append / vconcat / mapcar ==
(qa (reverse "abc") "cba")
(qa (reverse "aéb") "béa")
;; string-reverse is void at file top but autoloaded by this point:
(qa (string-reverse "abc") "cba")
(qa (string-reverse "aéb") "béa")

(qa (concat) "")
(qa (concat "ab" [?c]) "abc")
(qa (concat "a" '(98)) "ab")
;; all-unibyte args → unibyte result (bytes stay bytes; chars 97 195 169):
(qa (concat "a" (unibyte-string #xc3 #xa9)) "a\303\251")
(qa (multibyte-string-p (concat "a" "\303\251")) nil)
;; a multibyte arg promotes the result, decoding the unibyte bytes as UTF-8:
(qa (concat (unibyte-string 97) "é") "aé")
(qa-err-data (concat "a" 1) 'wrong-type-argument '(sequencep 1))

(qa (append "ab" '(?c)) '(97 98 99))
(qa (append "ab" "cd") '(97 98 . "cd"))
(qa (vconcat "ab" "cd") [97 98 99 100])
(qa (vconcat "ab" '(?c ?d)) [97 98 99 100])
(qa (mapcar #'upcase "abc") '(65 66 67))
(qa (mapconcat #'upcase '("a" "b") "-") "A-B")

;; == element access / construction ==
(qa (aref "abc" 1) 98)
(let ((qas-s (copy-sequence "abc")))
  (qa (progn (aset qas-s 1 ?z) qas-s) "azc"))
(qa-err-data (aset "abc" 5 ?x) 'args-out-of-range '("abc" 5))
(let ((qas-s (copy-sequence "abcdef")))
  (qa (progn (store-substring qas-s 1 "XY") qas-s) "aXYdef"))
(qa (length "héllo") 5)
(qa (length "日本") 2)
(qa (make-string 3 ?x) "xxx")
(qa (make-string 0 ?a) "")
(qa (char-to-string ?a) "a")
(qa (string ?a ?b 99) "abc")
(qa (string-to-char "abc") 97)
(qa (string-to-char "") 0)

;; == multibyte / unibyte ==
(qa (multibyte-string-p "é") t)
(qa (multibyte-string-p "abc") nil)
(qa (multibyte-string-p (unibyte-string 97)) nil)
(qa (string-to-unibyte "abc") "abc")
(qa-err-data (string-to-unibyte "é") 'error
             '("Cannot convert character at index 0 to unibyte"))
(qa (string-to-multibyte (unibyte-string 97 98)) "ab")
(qa (multibyte-string-p (string-to-multibyte "a")) t)
(qa (string-to-unibyte (string-to-multibyte "\303\251")) "\303\251")
(qa (string-as-unibyte "é") "\303\251")
(qa (string-as-multibyte "\303\251") "é")
(qa (string-make-unibyte "é") "\351")
(qa (multibyte-string-p (string-make-multibyte "\303\251")) t)
(qa (length (string-make-multibyte "\303\251")) 2)

;; == coding-system roundtrips ==
(qa (encode-coding-string "é" 'utf-8) "\303\251")
(qa (encode-coding-string "abc" 'us-ascii) "abc")
(qa (encode-coding-string "é" 'us-ascii) "?")
(qa (encode-coding-string "é" 'iso-8859-1) "\351")
(qa (decode-coding-string (encode-coding-string "héllo" 'utf-8) 'utf-8)
    "héllo")
(qa (decode-coding-string (encode-coding-string "日本" 'utf-8) 'utf-8)
    "日本")
(qa (decode-coding-string (encode-coding-string "é" 'iso-8859-1) 'iso-8859-1)
    "é")

;; == string<->number ==
(qa (string-to-number "12") 12)
(qa (string-to-number "12x") 12)
(qa (string-to-number "x") 0)
(qa (string-to-number "-12") -12)
(qa (string-to-number "+7") 7)
(qa (string-to-number " 12") 12)
(qa (string-to-number "1.5") 1.5)
(qa (string-to-number ".5") 0.5)
(qa (string-to-number "-.5") -0.5)
(qa (string-to-number "1e3") 1000.0)
(qa (string-to-number "") 0)
(qa (string-to-number "#xff") 0)
(qa (string-to-number "ff" 16) 255)
(qa (string-to-number "101" 2) 5)
(qa-err-data (string-to-number "ff" 1) 'args-out-of-range '(1))
(qa-err-data (string-to-number "17" 0) 'args-out-of-range '(0))
(qa (number-to-string 42) "42")
(qa (number-to-string -7) "-7")
(qa (number-to-string 1.5) "1.5")
(qa (number-to-string 0.1) "0.1")

;; == format ==
(qa (format "%d" 42) "42")
(qa (format "%d" 2.9) "2")
(qa (format "%o" 64) "100")
(qa (format "%x" 255) "ff")
(qa (format "%X" 255) "FF")
(qa (format "%c" ?a) "a")
(qa (format "%c" ?é) "é")
(qa (format "%%") "%")
(qa (format "%05d" 42) "00042")
(qa (format "%-8d|" 42) "42      |")
(qa (format "%-5s|" "ab") "ab   |")
(qa (format "%10s" "hi") "        hi")
(qa (format "%g" 1.5) "1.5")
(qa (format "%g" 100000.0) "100000")
(qa (format "%g" 0.00001) "1e-05")
(qa (format "%e" 1234.5) "1.234500e+03")
(qa (format "%f" 1.5) "1.500000")
(qa (format "%.2f" 3.14159) "3.14")
(qa (format "%.0f" 2.5) "2")
(qa (format "%+.2f" 3.14159) "+3.14")
(qa (format "%s" 'foo) "foo")
(qa (format "%s" nil) "nil")
(qa (format "%s" t) "t")
(qa (format "%s" 1.5) "1.5")
(qa (format "%s" ?a) "97")
(qa (format "%s" '(1 2)) "(1 2)")
(qa (format "%s" [1 2]) "[1 2]")
(qa (format "%S" "a\nb") "\"a\nb\"")
(qa (format "x" 1 2) "x")
(qa-err (format "%s %s" "a") 'error)
(qa-err (format "%c" "a") 'error)
(qa-err (format "%d" 'x) 'error)
(qa-err (format "%*d" 5 42) 'error)

;; == hashing ==
(qa (secure-hash 'md5 "hello") "5d41402abc4b2a76b9719d911017c592")
(qa (secure-hash 'sha1 "hello") "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d")
(qa (secure-hash 'sha256 "hello")
    "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
(qa (md5 "hello") "5d41402abc4b2a76b9719d911017c592")
(qa (natnump (sxhash "abc")) t)
(qa (= (sxhash "abc") (sxhash "abc")) t)

(princ (qa--done))
