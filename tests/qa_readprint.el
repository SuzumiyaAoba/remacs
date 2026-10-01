;; readprint — Emacs Lisp reader (`read', `read-from-string') and printer
;; (`prin1-to-string', `with-output-to-string', print-* variables).
;; GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - `1.' reads as the integer 1; `0x10' / `1a' / `..' read as
;;     SYMBOLS, not numbers.
;;   - `|' is an ordinary symbol constituent (NO vertical-bar escape).
;;   - `##' / `#:' read as the uninterned empty-name symbol.
;;   - Chars print as their integer code (prin1 ?a → "97").
;;   - print-gensym defaults to nil.
;;   - `readablep' returns the PRINTED STRING (not the object).
;;   - `read' on a buffer advances point.
;;
;; NOTE: `qa' evals each form dynamically — bind locals INSIDE the
;; checked form (a lexical `let' outside `qa' is invisible to eval).

(defun qarp-r (s) (car (read-from-string s)))

;; == integers ==
(qa (qarp-r "42") 42)
(qa (qarp-r "+42") 42)
(qa (qarp-r "-42") -42)
(qa (qarp-r "042") 42)
(qa (qarp-r "#xFF") 255)
(qa (qarp-r "#Xff") 255)
(qa (qarp-r "#x-ff") -255)
(qa (qarp-r "#o17") 15)
(qa (qarp-r "#b101") 5)
(qa (qarp-r "#2r-11") -3)
(qa (qarp-r "#8r17") 15)
(qa (qarp-r "#16rff") 255)
(qa (qarp-r "#36rz") 35)
(qa-err (qarp-r "#37rZ") 'invalid-read-syntax)
(qa-err (qarp-r "#xg") 'invalid-read-syntax)
(qa-err (qarp-r "#o8") 'invalid-read-syntax)
(qa-err (qarp-r "#x") 'invalid-read-syntax)

;; == floats ==
(qa (qarp-r "1.0") 1.0)
(qa (qarp-r ".5") 0.5)
(qa (qarp-r "1.") 1)
(qa (qarp-r "1e3") 1000.0)
(qa (qarp-r "1E-2") 0.01)
(qa (qarp-r "+1.5") 1.5)
(qa (qarp-r "-.5") -0.5)
(qa (qarp-r "+.5e2") 50.0)
(qa (qarp-r "1e999") 1.0e+INF)
(qa (qarp-r "-1e999") -1.0e+INF)
(qa (qarp-r "-0.0") -0.0)

;; == tokens that look numeric but read as symbols ==
(qa (qarp-r "0x") '0x)
(qa (symbol-name (qarp-r "0x10")) "0x10")
(qa (qarp-r "0b101") '0b101)
(qa (qarp-r "1+") '1+)
(qa (qarp-r "++") '++)
(qa (qarp-r "-") '-)
(qa (qarp-r "+") '+)
(qa (qarp-r "/") '/)
(qa (qarp-r "1e") '1e)
(qa (qarp-r "..") '..)
(qa (qarp-r "...") '...)
(qa (qarp-r "1a") '1a)
(qa (symbol-name (qarp-r ".5a")) ".5a")
(qa (qarp-r "NIL") 'NIL)
(qa (eq (qarp-r "NIL") nil) nil)
(qa (eq (qarp-r "nil") nil) t)

;; == characters ==
(qa (qarp-r "?a") 97)
(qa (qarp-r "?A") 65)
(qa (qarp-r "?\\n") 10)
(qa (qarp-r "?\\t") 9)
(qa (qarp-r "?\\s") 32)
(qa (qarp-r "?\\\\") 92)
(qa (qarp-r "?\\d") 127)
(qa (qarp-r "?\\e") 27)
(qa (qarp-r "?\\a") 7)
(qa (qarp-r "?\\C-a") 1)
(qa (qarp-r "?\\C-?") 127)
(qa (qarp-r "?\\^I") 9)
(qa (qarp-r "?\\M-x") 134217848)
(qa (qarp-r "?\\M-\\C-a") 134217729)
(qa (qarp-r "?\\S-a") 33554529)
(qa (qarp-r "?\\s-b") 8388706)
(qa (qarp-r "?\\N{LATIN CAPITAL LETTER A}") 65)
(qa (qarp-r "?\\N{EURO SIGN}") 8364)
(qa (qarp-r "?\\x41") 65)
(qa (qarp-r "?\\u0041") 65)
(qa (qarp-r "?é") 233)
(qa-err (qarp-r "?ab") 'invalid-read-syntax)
(qa-err (qarp-r "?") 'end-of-file)

;; == strings ==
(qa (qarp-r "\"abc\"") "abc")
(qa (qarp-r "\"\"") "")
(qa (append (qarp-r "\"a\\nb\"") nil) '(97 10 98))
(qa (append (qarp-r "\"\\t\\r\\f\\v\"") nil) '(9 13 12 11))
(qa (append (qarp-r "\"\\a\\b\\e\"") nil) '(7 8 27))
(qa (append (qarp-r "\"\\d\"") nil) '(127))
(qa (append (qarp-r "\"\\0\"") nil) '(0))
(qa (append (qarp-r "\"\\012\"") nil) '(10))
(qa (append (qarp-r "\"\\x41\"") nil) '(65))
(qa (append (qarp-r "\"\\u0041\"") nil) '(65))
(qa (append (qarp-r "\"\\N{LATIN CAPITAL LETTER A}\"") nil) '(65))
(qa (qarp-r "\"\\\"\"") "\"")
(qa (qarp-r "\"a\\\nb\"") "ab")
(qa (qarp-r "\"a\\\n  b\"") "a  b")
(qa-err (qarp-r "\"unterminated") 'end-of-file)
(qa-err (qarp-r "\"\\x\"") 'error)

;; == symbols ==
(qa (qarp-r "a\\ b") (intern "a b"))
(qa (symbol-name (qarp-r "a\\ b")) "a b")
;; `|' is an ordinary constituent in 31.1 — "|weird sym|" splits
(qa (symbol-name (qarp-r "|weird sym|")) "|weird")
(qa (symbol-name (qarp-r "||")) "||")
(qa (symbol-name (qarp-r "a|b|c")) "a|b|c")
;; #: reads an uninterned symbol
(qa (symbol-name (qarp-r "#:foo")) "foo")
(qa (eq (qarp-r "#:foo") 'foo) nil)
(qa (eq (qarp-r "#:x") (qarp-r "#:x")) nil)
;; ## and #: alone read as the uninterned empty-name symbol
(qa (symbol-name (qarp-r "##")) "")
(qa (eq (qarp-r "##") (qarp-r "##")) t)
(qa (symbol-name (qarp-r "#:")) "")
(qa (qarp-r ":key") :key)

;; == quote, backquote, comma reader forms ==
(qa (qarp-r "'x") '(quote x))
(qa (qarp-r "#'f") '(function f))
(qa (qarp-r "'()") '(quote nil))
(qa (qarp-r "`x") '(\` x))
(qa (qarp-r "`(a ,b ,@c)") '(\` (a (\, b) (\,@ c))))
(qa (qarp-r "(a ,b)") '(a (\, b)))
(qa-err (qarp-r "'") 'end-of-file)
(qa-err (qarp-r "`") 'end-of-file)
(qa-err (qarp-r "#'") 'end-of-file)

;; == lists, dotted pairs, misc read errors ==
(qa (qarp-r "()") nil)
(qa (qarp-r "(nil)") '(nil))
(qa (qarp-r "(a . b)") '(a . b))
(qa (qarp-r "(a b . c)") '(a b . c))
;; trailing bare dot reads as the symbol `\.'
(qa (qarp-r "(a .)") '(a \.))
(qa-err (qarp-r "(a . b c)") 'invalid-read-syntax)
(qa-err (qarp-r ")") 'invalid-read-syntax)
(qa-err (qarp-r "(") 'end-of-file)
(qa-err (qarp-r "[1 2)") 'invalid-read-syntax)

;; == vectors, records, bool-vectors, hash literals ==
(qa (qarp-r "[1 2 \"x\"]") [1 2 "x"])
(qa (qarp-r "[]") [])
(qa (qarp-r "#s(foo a b)") (record 'foo 'a 'b))
(qa-err (qarp-r "#(1 2)") 'invalid-read-syntax)
(qa (qarp-r "#&3\"\\x05\"") (bool-vector t nil t))
(qa (let ((h (car (read-from-string "#s(hash-table data (a 1 b 2))"))))
      (list (hash-table-p h)
            (gethash 'a h)
            (gethash 'b h)
            (hash-table-count h)))
    '(t 1 2 2))
(qa (hash-table-test (qarp-r "#s(hash-table test eql data (1.5 x))"))
    'eql)
;; text properties via #(...)
(qa (get-text-property 0 'a (qarp-r "#(\"xy\" 0 2 (a 1))")) 1)

;; == read-from-string: return value, start/end, eof ==
(qa-err (read-from-string "") 'end-of-file)
(qa-err (read-from-string "   ") 'end-of-file)
(qa-err (read-from-string " ; only comment") 'end-of-file)
(qa (read-from-string " 42") '(42 . 3))
(qa (read-from-string "  42 99") '(42 . 4))
(qa (read-from-string "(a . b) xyz") '((a . b) . 7))
(qa (read-from-string "a b") '(a . 1))
(qa (read-from-string "x ;c\ny") '(x . 1))
(qa (read-from-string "; comment\n42") '(42 . 12))
(qa (read-from-string "ab" 1) '(b . 2))
(qa (read-from-string "xx 42" 3) '(42 . 5))
(qa (read-from-string "ab 42" 3 5) '(42 . 5))
(qa-err (read-from-string "ab" 2) 'end-of-file)
(qa-err (read-from-string "ab" 3) 'args-out-of-range)

;; == read on a string / on a buffer ==
(qa (read "\"hello\"") "hello")
(qa (read "'x") '(quote x))
(qa (read "()") nil)
;; read from a buffer consumes text and advances point
(qa (with-temp-buffer
      (insert "(a b) 42")
      (goto-char (point-min))
      (list (read (current-buffer)) (point)
            (read (current-buffer)) (point)))
    '((a b) 6 42 9))
;; read-print roundtrip via buffer insert + read
(qa (with-temp-buffer
      (insert (prin1-to-string '(1 "two" [3] (4 . 5))))
      (goto-char (point-min))
      (read (current-buffer)))
    '(1 "two" [3] (4 . 5)))

;; == #n= / #n# shared and circular structure ==
(qa (let ((x (car (read-from-string "#1=(a . #1#)"))))
      (eq x (cdr x)))
    t)
(qa (let ((x (car (read-from-string "(#1=(1 2) #1#)"))))
      (eq (car x) (cadr x)))
    t)
(qa-err (qarp-r "(a #1#)") 'invalid-read-syntax)

;; == printing: prin1-to-string ==
(qa (prin1-to-string "abc") "\"abc\"")
(qa (prin1-to-string "a\nb") "\"a\nb\"")
(qa (prin1-to-string "he\"llo\\") "\"he\\\"llo\\\\\"")
(qa (prin1-to-string 'sym) "sym")
(qa (prin1-to-string (intern "a b")) "a\\ b")
(qa (prin1-to-string [1 "a" ?b]) "[1 \"a\" 98]")
(qa (prin1-to-string '(1 . 2)) "(1 . 2)")
(qa (prin1-to-string '(1 2 . 3)) "(1 2 . 3)")
(qa (prin1-to-string (record 'a 1 2)) "#s(a 1 2)")
(qa (prin1-to-string '(quote x)) "'x")
(qa (prin1-to-string '(function f)) "#'f")
(qa (prin1-to-string nil) "nil")
;; chars print as integer codes
(qa (prin1-to-string ?a) "97")
(qa (prin1-to-string ?\C-a) "1")
(qa (prin1-to-string ?\M-x) "134217848")
(qa (prin1-to-string ?é) "233")
(qa (prin1-to-string most-positive-fixnum) "2305843009213693951")
(qa (prin1-to-string most-negative-fixnum) "-2305843009213693952")
(qa (prin1-to-string 1.5) "1.5")
(qa (prin1-to-string .5) "0.5")
(qa (prin1-to-string 1e999) "1.0e+INF")
(qa (prin1-to-string -0.0) "-0.0")

;; == print-gensym / print-quoted ==
(qa print-gensym nil)
(qa (prin1-to-string (make-symbol "gs")) "gs")
(qa (let ((print-gensym t)) (prin1-to-string (make-symbol "gs")))
    "#:gs")
(qa (let ((print-quoted nil)) (prin1-to-string '(quote x)))
    "(quote x)")
(qa (let ((print-quoted nil)) (prin1-to-string '(function f)))
    "(function f)")

;; == print-escape-* ==
(qa (let ((print-escape-newlines t)) (prin1-to-string "a\nb"))
    "\"a\\nb\"")
(qa (let ((print-escape-control-characters t))
      (prin1-to-string "a\tb\177"))
    "\"a\\11b\\177\"")

;; == print-circle / print-length / print-level ==
(qa (let ((x (list 1)))
      (setcdr x x)
      (let ((print-circle t)) (prin1-to-string x)))
    "#1=(1 . #1#)")
(qa (let ((c (list 1 2)) (print-circle t))
      (prin1-to-string (list c c)))
    "(#1=(1 2) #1#)")
(qa (let ((c (list 1 2)) (print-circle nil))
      (prin1-to-string (list c c)))
    "((1 2) (1 2))")
(qa (let ((print-length 2)) (prin1-to-string '(1 2 3 4))) "(1 2 ...)")
(qa (let ((print-length 0)) (prin1-to-string '(1 2))) "(...)")
(qa (let ((print-length 2)) (prin1-to-string [1 2 3 4])) "[1 2 ...]")
(qa (let ((print-level 1)) (prin1-to-string '((a) (b)))) "(... ...)")
(qa (let ((print-level 2)) (prin1-to-string '((a (b)) c))) "((a ...) c)")

;; == readablep ==
;; returns the printed STRING when the object has read syntax
;; remacs: returns the object, not the printed string
(qa (readablep 42) "42")
(qa (readablep "s") "\"s\"")
(qa (readablep 'a) "a")
(qa (readablep '(a . b)) "(a . b)")
(qa (readablep [1 2]) "[1 2]")
;; unreadable objects → nil
(qa (readablep (symbol-function 'car)) nil)
(qa (readablep (current-buffer)) nil)

;; == with-output-to-string ==
(qa (with-output-to-string (princ "hi") (prin1 '(1 2))) "hi(1 2)")
(qa (with-output-to-string (print 'a)) "\na\n")
(qa (with-output-to-string (princ "a\nb")) "a\nb")
(qa (with-output-to-string (prin1 "a\nb")) "\"a\nb\"")
(qa (with-output-to-string (terpri) (terpri) (princ "x")) "\n\nx")

;; == printed forms of buffers and markers ==
(qa (buffer-name) "*scratch*")
(qa (prin1-to-string (get-buffer-create "qarp-buf")) "#<buffer qarp-buf>")
(qa (with-current-buffer (get-buffer-create "qarp-m")
      (prin1-to-string (point-min-marker)))
    "#<marker at 1 in qarp-m>")

(princ (qa--done))
