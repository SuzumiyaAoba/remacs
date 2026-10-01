;; json — Lisp reader/encoder (json.el) plus native jansson bindings.
;; GNU-verified 31.1.  Note: in 31.1 `json-false' is :json-false and
;; `json-null' is nil; `json-new-object' returns nil.
(require 'json)

;; == lisp reader: scalars ==
(qa (json-read-from-string "42") 42)
(qa (json-read-from-string "-7") -7)
(qa (json-read-from-string "1.5") 1.5)
(qa (json-read-from-string "-0.25") -0.25)
(qa (json-read-from-string "1e3") 1000.0)
(qa (json-read-from-string "1E-3") 0.001)
(qa (json-read-from-string "12345678901234567890123") 12345678901234567890123)
(qa (json-read-from-string "  5  ") 5)
(qa (json-read-from-string "true") t)
(qa (json-read-from-string "false") :json-false)
(qa (json-read-from-string "null") nil)
;; GNU quirk: keyword comparison is case-insensitive, so "nulL" reads as null.
(qa (json-read-from-string "nulL") nil)
(qa-same json-false :json-false)
(qa json-null nil)
(qa (json-new-object) nil)

;; == lisp reader: strings ==
(qa (json-read-from-string "\"plain\"") "plain")
(qa (json-read-from-string "\"a\\nb\\tc\"") "a\nb\tc")
(qa (json-read-from-string "\"\\u0041\"") "A")
(qa (json-read-from-string "\"a\\/b\"") "a/b")
(qa (json-read-from-string "\"\\\" \\\\ \\/ \\b \\f \\n \\r \\t\"")
    "\" \\ / \b \f \n \r \t")
(let ((qaJ-s (json-read-from-string "\"\\ud83d\\ude00\"")))
  (qa (length qaJ-s) 1)
  (qa (aref qaJ-s 0) 128512))
;; GNU quirk: a lone surrogate escape yields the raw surrogate char.
;; remacs stores surrogates as plane-15 PUA proxies (`lisp_char')
;; which read back as their real codes.
(let ((qaJ-s (json-read-from-string "\"\\ud83d\"")))
  (qa (length qaJ-s) 1)
  (qa (aref qaJ-s 0) 55357))

;; == lisp reader: objects, arrays, key/array types ==
(qa (json-read-from-string "{\"a\":1}") '((a . 1)))
(qa (json-read-from-string "{}") nil)
(qa (json-read-from-string "{\"a b\":1,\"c-d\":2}") '((a\ b . 1) (c-d . 2)))
(qa (let ((json-key-type 'string)) (json-read-from-string "{\"a\":1}"))
    '(("a" . 1)))
(qa (let ((json-key-type 'keyword)) (json-read-from-string "{\"a\":1}"))
    '((:a . 1)))
(qa (let ((json-object-type 'plist)) (json-read-from-string "{\"a\":1}"))
    '(:a 1))
(let ((qaJ-h (let ((json-object-type 'hash-table))
               (json-read-from-string "{\"a\":1}"))))
  (qa (hash-table-test qaJ-h) 'equal)
  (qa (hash-table-count qaJ-h) 1)
  (qa (gethash "a" qaJ-h) 1))
(qa (json-read-from-string "[1,2]") [1 2])
(qa (let ((json-array-type 'list)) (json-read-from-string "[1,2]")) '(1 2))
(qa (json-read-from-string "{\"a\":{\"b\":[1,{\"c\":null}],\"d\":false}}")
    '((a (b . [1 ((c))]) (d . :json-false))))
;; GNU quirk: a trailing comma in an array is silently tolerated.
(qa (json-read-from-string "[1,]") [1])
(qa (json-read-from-string "  \t\n [ 1 , 2 ] \r\n") [1 2])
;; GNU quirk: duplicate keys are all kept in the alist.
(qa (json-read-from-string "{\"a\":1,\"a\":2}") '((a . 1) (a . 2)))

;; == lisp reader: errors ==
(qa-err (json-read-from-string "") 'json-end-of-file)
(qa-err (json-read-from-string "{") 'end-of-buffer)
(qa-err (json-read-from-string "{bad") 'json-end-of-file)
(qa-err (json-read-from-string "[1,") 'json-end-of-file)
(qa-err (json-read-from-string "\"unterminated") 'json-end-of-file)
(qa-err (json-read-from-string "{a:1}") 'json-end-of-file)
(qa-err (json-read-from-string "{,}") 'json-end-of-file)
(qa-err-data (json-read-from-string "{\"a\":}") 'json-readtable-error '(125))
(qa-err (json-read-from-string "01") 'json-number-format)
(qa-err-data (json-read-from-string "1.") 'json-number-format '(2))
(qa-err (json-read-from-string ".5") 'json-readtable-error)
(qa-err (json-read-from-string "+") 'json-readtable-error)
(qa-err (json-read-from-string "'x'") 'json-readtable-error)
(qa-err (json-read-from-string "[1 2]") 'json-array-format)
(qa-err (json-read-from-string "tru") 'json-unknown-keyword)
(qa-err-data (json-read-from-string "truex") 'json-unknown-keyword '("truex"))
;; GNU quirk: content after the first complete value is ignored.
(qa (json-read-from-string "{\"a\":1} garbage") '((a . 1)))

;; == json-read from a buffer ==
(let ((qaJ-buf (generate-new-buffer " *qaJ*")))
  (unwind-protect
      (with-current-buffer qaJ-buf
        (insert "junk {\"x\":[1,2]} tail")
        (goto-char 6)
        (qa (json-read) '((x . [1 2])))
        (qa (point) 17))
    (kill-buffer qaJ-buf)))
(let ((qaJ-buf (generate-new-buffer " *qaJ*")))
  (unwind-protect
      (with-current-buffer qaJ-buf
        (insert "{\"a\":1}")
        (qa-err (progn (goto-char (point-max)) (json-read)) 'json-end-of-file))
    (kill-buffer qaJ-buf)))

;; == encoding ==
(qa (json-encode 1) "1")
(qa (json-encode -3) "-3")
(qa (json-encode 1.5) "1.5")
(qa (json-encode 1.0) "1.0")
(qa (json-encode -0.0) "-0.0")
(qa (json-encode 12345678901234567890123) "12345678901234567890123")
;; GNU quirk: non-finite floats print in the C printf style.
(qa (json-encode (/ 1.0 0.0)) "1.0e+INF")
(qa (json-encode ?x) "120")
(qa (json-encode t) "true")
(qa (json-encode :json-false) "false")
(qa (json-encode nil) "null")
(qa (json-encode 'foo) "\"foo\"")
(qa (json-encode (intern "weird sym")) "\"weird sym\"")
(qa (json-encode :a) "\"a\"")
(qa (json-encode '(1 2)) "[1,2]")
(qa (json-encode '(a b)) "[\"a\",\"b\"]")
(qa (json-encode [1 2 3]) "[1,2,3]")
(qa (json-encode [t :json-false nil]) "[true,false,null]")
;; A list starting with a keyword is taken for a plist, not an array.
(qa (json-encode '(:a 1)) "{\"a\":1}")
(qa (json-encode '((a . (1 2)) (b . "x"))) "{\"a\":[1,2],\"b\":\"x\"}")
(qa-err (json-encode '(a . b)) 'wrong-type-argument)
(qa-err (json-encode-list '(a . b)) 'wrong-type-argument)
(qa-err-data (json-encode-plist '(1 2)) 'json-key-format '(1))

(qa (json-encode-string "a\"b\\c\nd") "\"a\\\"b\\\\c\\nd\"")
(qa (json-encode-string "a\tb") "\"a\\tb\"")
(qa (json-encode-string "a\001b\177c") "\"a\\u0001b\177c\"")
(qa (json-encode-string "a/b") "\"a/b\"")
(qa (json-encode-string "\u00e9\U0001f600") "\"\u00e9\U0001f600\"")

(qa (json-encode-list '(1 2 3)) "[1,2,3]")
(qa (json-encode-list nil) "{}")
(qa (json-encode-alist '((a . 1) (b . "x"))) "{\"a\":1,\"b\":\"x\"}")
(qa (json-encode-alist '(("s" . 1))) "{\"s\":1}")
(qa (json-encode-alist '((:a . 1))) "{\"a\":1}")
(qa (json-encode-alist '((on . t) (off . :json-false)))
    "{\"on\":true,\"off\":false}")
(qa (json-encode-alist nil) "{}")
(qa (json-encode-alist '((k . (1 2)))) "{\"k\":[1,2]}")
(qa (json-encode-plist '(:a 1 :b "x")) "{\"a\":1,\"b\":\"x\"}")
(qa (json-encode (json-new-object)) "null")
(let ((qaJ-h (make-hash-table :test 'equal)))
  (puthash "only" 5 qaJ-h)
  (qa (json-encode qaJ-h) "{\"only\":5}"))

;; == round trip through the lisp reader ==
(qa (json-read-from-string (json-encode '((a . (1 2)) (b . "x"))))
    '((a . [1 2]) (b . "x")))
(qa (json-read-from-string (json-encode [1 "s" t nil :json-false]))
    [1 "s" t nil :json-false])

;; == json-read-file ==
(let ((qaJ-f (make-temp-file "qaJ" nil nil "{\"k\": [1, 2], \"s\": \"v\"}")))
  (unwind-protect
      (qa (json-read-file qaJ-f) '((k . [1 2]) (s . "v")))
    (delete-file qaJ-f)))

;; == json-pretty-print ==
(let ((qaJ-buf (generate-new-buffer " *qaJ*")))
  (unwind-protect
      (with-current-buffer qaJ-buf
        (insert "{\"a\":1,\"b\":[2,3]}")
        (json-pretty-print (point-min) (point-max))
        (qa (buffer-string) "{\n  \"a\": 1,\n  \"b\": [\n    2,\n    3\n  ]\n}"))
    (kill-buffer qaJ-buf)))
(let ((qaJ-buf (generate-new-buffer " *qaJ*")))
  (unwind-protect
      (with-current-buffer qaJ-buf
        (insert "{\"a\":1}")
        (json-pretty-print-buffer)
        (qa (buffer-string) "{\n  \"a\": 1\n}"))
    (kill-buffer qaJ-buf)))
(let ((qaJ-buf (generate-new-buffer " *qaJ*")))
  (unwind-protect
      (with-current-buffer qaJ-buf
        (insert "{\n  \"a\": 1,\n  \"b\": 2\n}")
        (json-pretty-print (point-min) (point-max) t)
        (qa (buffer-string) "{\"a\":1,\"b\":2}"))
    (kill-buffer qaJ-buf)))

;; == native jansson bindings ==
(when (fboundp 'json-parse-string)
  ;; default object type is a hash table with string keys.
  (let ((qaJ-h (json-parse-string "{\"a\":1}")))
    (qa (hash-table-test qaJ-h) 'equal)
    (qa (gethash "a" qaJ-h) 1)
    (qa (gethash 'a qaJ-h) nil)
    (qa (hash-table-count (json-parse-string "{\"a\":1,\"b\":2}")) 2))
  (qa-prin1 (json-parse-string "{\"a\":1}")
            "#s(hash-table test equal data (\"a\" 1))")
  ;; hash tables do not compare with `equal'.
  (qa (equal (json-parse-string "{\"a\":1}")
             (json-parse-string "{\"a\":1}"))
      nil)
  (qa (json-parse-string "{\"a\":1}" :object-type 'alist) '((a . 1)))
  (qa (json-parse-string "{\"a\":1}" :object-type 'plist) '(:a 1))
  (qa (json-parse-string "{}" :object-type 'alist) nil)
  (qa (json-parse-string "[1,2]" :array-type 'list) '(1 2))
  (qa (json-parse-string "[1,2]") [1 2])
  (qa (json-parse-string "1.5") 1.5)
  (qa (json-parse-string "1e3") 1000.0)
  (qa (json-parse-string "42") 42)
  (qa (json-parse-string "true") t)
  (qa (json-parse-string "\"s\"") "s")
  (qa (json-parse-string "\"\\u0041\\ud83d\\ude00\"") "A\U0001f600")
  (qa (json-parse-string "{\"o\":{\"n\":[1,[2]]}}" :object-type 'alist)
      '((o (n . [1 [2]]))))
  (qa (json-parse-string "[true,false,null]") [t :false :null])
  (qa (json-parse-string "[true,false,null]" :null-object :NULL
                         :false-object :FALSE)
      [t :FALSE :NULL])
  (qa (json-parse-string "{\"a\":1,\"a\":2}" :object-type 'alist)
      '((a . 1) (a . 2)))
  (qa (json-parse-string "   [7]") [7])

  (qa-err-data (json-parse-string "{bad") 'json-parse-error '(1 nil 2))
  (qa-err-data (json-parse-string "") 'json-end-of-file '(1 nil 0))
  (qa-err-data (json-parse-string "   ") 'json-end-of-file '(1 nil 3))
  (qa-err-data (json-parse-string "[1,") 'json-end-of-file '(1 nil 3))
  (qa-err-data (json-parse-string "notjson") 'json-parse-error '(1 nil 2))
  (qa-err-data (json-parse-string "{\"a\":1} extra")
               'json-trailing-content '(1 nil 9))
  (qa-err (json-parse-string "1" :bogus t) 'error)

  (let ((qaJ-buf (generate-new-buffer " *qaJ*")))
    (unwind-protect
        (with-current-buffer qaJ-buf
          (insert "zz {\"q\":2}")
          (goto-char 4)
          (qa (gethash "q" (json-parse-buffer)) 2)
          (goto-char 4)
          (json-parse-buffer)
          (qa (point) 11))
      (kill-buffer qaJ-buf)))

  ;; error POSITION is the char offset relative to the parse start,
  ;; and point at end-of-buffer reports 0 (EOF at parse start).
  (let ((qaJ-buf (generate-new-buffer " *qaJ*")))
    (unwind-protect
        (with-current-buffer qaJ-buf
          (insert "{bad")
          (qa-err-data (json-parse-buffer) 'json-end-of-file '(1 nil 0))
          (goto-char (point-min))
          (qa-err-data (json-parse-buffer) 'json-parse-error '(1 nil 2))
          (erase-buffer)
          (insert "tru")
          (goto-char (point-min))
          ;; a truncated literal is json-parse-error (not EOF), and
          ;; POSITION counts the consumed chars.
          (qa-err-data (json-parse-buffer) 'json-parse-error '(1 nil 3)))
      (kill-buffer qaJ-buf)))
  ;; GNU's literal check also rejects a token char right after the
  ;; keyword (json.c json_is_token_char), unlike json.el's reader.
  (qa-err-data (json-parse-string "truex") 'json-parse-error '(1 nil 5))
  (qa-err-data (json-parse-string "tru") 'json-parse-error '(1 nil 3))

  ;; == json-serialize ==
  (qa (json-serialize '((a . 1) (b . "x"))) "{\"a\":1,\"b\":\"x\"}")
  (qa (json-serialize '(:a 1 :b 2)) "{\"a\":1,\"b\":2}")
  (qa (json-serialize '((a . 1))) "{\"a\":1}")
  (qa (json-serialize [1 2]) "[1,2]")
  (qa (json-serialize "s") "\"s\"")
  (qa (json-serialize 1.5) "1.5")
  (qa (json-serialize 12345678901234567890123) "12345678901234567890123")
  (qa (json-serialize t) "true")
  (qa (json-serialize :false) "false")
  (qa (json-serialize :null) "null")
  ;; nil is the empty object for the native serializer.
  (qa (json-serialize nil) "{}")
  (qa (json-serialize ?x) "120")
  (qa (json-serialize "a\"b\nc") "\"a\\\"b\\nc\"")
  (qa (json-serialize "a\001b\177c") "\"a\\u0001b\177c\"")
  (qa (json-serialize "\b\f") "\"\\b\\f\"")
  ;; native output is a unibyte UTF-8 string (unlike json-encode).
  (let ((qaJ-s (json-serialize "\u00e9\U0001f600")))
    (qa (multibyte-string-p qaJ-s) nil)
    (qa (length qaJ-s) 8)
    (qa (decode-coding-string qaJ-s 'utf-8) "\"\u00e9\U0001f600\""))
  (let ((qaJ-h (make-hash-table :test 'equal)))
    (puthash "only" 5 qaJ-h)
    (qa (json-serialize qaJ-h) "{\"only\":5}"))
  ;; a bare list is an object, not an array: its car must be a symbol.
  (qa-err-data (json-serialize '(1 2)) 'wrong-type-argument '(symbolp 1))
  (qa-err-data (json-serialize '(:k . :v)) 'wrong-type-argument '(consp :v))
  (qa-err-data (json-serialize '(("s" . 1))) 'wrong-type-argument '(symbolp "s"))
  (qa-err (json-serialize (make-marker)) 'wrong-type-argument)
  (qa (json-serialize '(:a :null) :null-object :null) "{\"a\":null}")
  (qa (json-serialize '(:a :FOO) :false-object :FOO) "{\"a\":false}")
  ;; full round trip through the alist object type is order-preserving.
  (qa (json-serialize
       (json-parse-string "{\"a\":[1,2.5,true,null]}" :object-type 'alist))
      "{\"a\":[1,2.5,true,null]}"))

(princ (qa--done))
