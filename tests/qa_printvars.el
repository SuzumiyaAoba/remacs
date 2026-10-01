;; print-circle / print-level / print-length / escape flags — GNU 31.1.
;;
;; GNU truths pinned (all verified against the Nix Emacs binary):
;;   - circular lists print with #N= / #N# when print-circle=t.
;;   - shared structure uses #N= only with print-circle=t.
;;   - print-level truncates nesting with "...".
;;   - print-length truncates list length.
;;   - prin1-to-string on a CHARACTER returns the digit string (e.g. "97"
;;     for ?a) — `?a' is a READER literal, not a print form.
;;   - print-escape-newlines controls \n; \t is NOT escaped by default.
;;   - print-escape-nonascii is a NO-OP in GNU.
;;   - print-escape-multibyte uses \xHHHH notation.
;;   - #: prefix on uninterned symbols requires print-gensym=t.
;;
;; `qa' evals its form in DYNAMIC scope — locals referenced inside a
;; `qa' form must be defvar'd (special) to be visible to `eval'.
(defvar l)
(defvar out)
(defvar s)

;; == surface ==
(qa (fboundp 'prin1) t)
(qa (fboundp 'prin1-to-string) t)
(qa (fboundp 'princ) t)
(qa (fboundp 'print) t)
(qa (fboundp 'terpri) t)
(qa (fboundp 'write-char) t)

;; == print-circle on circular list ==
(let ((l (list 1 2 3)))
  (setcdr (cddr l) l)
  ;; without print-circle — depth-bounded "(1 2 3 1 2 . #2)"
  (let ((print-circle nil))
    (qa (prin1-to-string l) "(1 2 3 1 2 . #2)"))
  ;; with print-circle — #N= notation
  (let ((print-circle t))
    (qa (prin1-to-string l) "#1=(1 2 3 . #1#)")))

;; == print-circle on shared (not circular) structure ==
(let* ((x (list 1))
       (l (list x x)))
  (let ((print-circle t))
    (qa (prin1-to-string l) "(#1=(1) #1#)"))
  (let ((print-circle nil))
    (qa (prin1-to-string l) "((1) (1))")))

;; == print-level truncation ==
(let ((print-level 2))
  (qa (prin1-to-string '((a (b c)) (d) e f)) "((a ...) (d) e f)"))
(let ((print-level 1))
  (qa (prin1-to-string '(a (b (c (d))))) "(a ...)"))

;; == print-length truncation ==
(let ((print-length 3))
  (qa (prin1-to-string '(1 2 3 4 5)) "(1 2 3 ...)"))
(let ((print-length 0))
  (qa (prin1-to-string '(1 2 3)) "(...)"))

;; == both combined ==
(let ((print-level 2) (print-length 2))
  (qa (prin1-to-string '((a (b)) (c (d)) (e (f)))) "((a ...) (c ...) ...)"))

;; == print-escape-newlines ==
(let ((print-escape-newlines t))
  (qa (prin1-to-string "a\nb") "\"a\\nb\""))
(let ((print-escape-newlines nil))
  (qa (prin1-to-string "a\nb") "\"a\nb\""))

;; == tab is NOT escaped by default ==
(qa (prin1-to-string "tab\t") "\"tab\t\"")

;; == print-escape-nonascii is a NO-OP in GNU ==
(let ((print-escape-nonascii nil))
  (qa (prin1-to-string "αβ") "\"αβ\""))
(let ((print-escape-nonascii t))
  ;; still literal — GNU doesn't use this flag for output
  (qa (prin1-to-string "αβ") "\"αβ\""))

;; == print-escape-multibyte DOES escape ==
(let ((print-escape-multibyte t))
  (qa (prin1-to-string "αβ") "\"\\x03b1\\x03b2\""))
(let ((print-escape-multibyte nil))
  (qa (prin1-to-string "αβ") "\"αβ\""))

;; == print-quoted — affects quote/backquote print ==
(let ((print-quoted t))
  (qa (prin1-to-string '(quote x)) "'x"))
(let ((print-quoted nil))
  (qa (prin1-to-string '(quote x)) "(quote x)"))
(let ((print-quoted t))
  (qa (prin1-to-string '`(a ,b ,@c)) "`(a ,b ,@c)"))
(let ((print-quoted nil))
  (qa (prin1-to-string '`(a ,b ,@c)) "(\\` (a (\\, b) (\\,@ c)))"))

;; == princ vs prin1 ==
(let ((out (with-output-to-string (princ "a\"b"))))
  (qa out "a\"b"))
(let ((out (with-output-to-string (prin1 "a\"b"))))
  (qa out "\"a\\\"b\""))

;; == terpri — emits newline ==
(let ((out (with-output-to-string (terpri))))
  (qa out "\n"))

;; == float print ==
(qa (prin1-to-string 3.5) "3.5")
(qa (prin1-to-string -0.5) "-0.5")
(qa (prin1-to-string 1e10) "10000000000.0")
(qa (prin1-to-string 0.0) "0.0")
(qa (prin1-to-string -0.0) "-0.0")

;; == string escapes ==
;; tab NOT escaped (verified above)
;; DEL and control chars print as literal bytes
(qa (prin1-to-string (string ?\x01)) "\"\x01\"")
(qa (prin1-to-string (string ?\x7f)) "\"\x7f\"")
;; double-quote and backslash escape
(qa (prin1-to-string "back\\") "\"back\\\\\"")
(qa (prin1-to-string "quote\"") "\"quote\\\"\"")

;; == symbol print ==
(qa (prin1-to-string 'foo) "foo")
;; a symbol NAME with a space needs the space escaped in source;
;; prin1 re-escapes it as "\ ".
(qa (prin1-to-string 'has\ space) "has\\ space")
;; uninterned symbol: print-gensym controls "#:"
(let ((s (make-symbol "gensym-name")))
  (let ((print-gensym nil))
    (qa (prin1-to-string s) "gensym-name"))
  (let ((print-gensym t))
    (qa (prin1-to-string s) "#:gensym-name")))

;; == vector print ==
(qa (prin1-to-string [1 2 3]) "[1 2 3]")
(qa (prin1-to-string []) "[]")

;; == char print — prints the integer value ==
(qa (prin1-to-string ?a) "97")
(qa (prin1-to-string ?\n) "10")
(qa (prin1-to-string ?\t) "9")
(qa (prin1-to-string ?\s) "32")
(qa (prin1-to-string ?\\) "92")
(qa (prin1-to-string ?\x7f) "127")

;; == hash-table print ==
(let ((h (make-hash-table :test 'eq)))
  (puthash 'a 1 h)
  (let ((s (prin1-to-string h)))
    (qa (string-prefix-p "#s(hash-table" s) t)))

;; == record print ==
(let ((r (record 'point 1 2)))
  (let ((s (prin1-to-string r)))
    (qa (string-prefix-p "#s(point" s) t)))

;; == print-level nil means unbounded ==
(let ((print-level nil))
  (qa (prin1-to-string '((a (b (c (d e)))))) "((a (b (c (d e)))))"))

;; == print-length nil — unbounded ==
(let ((print-length nil))
  (qa (prin1-to-string '(1 2 3 4 5 6 7 8 9 10))
      "(1 2 3 4 5 6 7 8 9 10)"))

;; == circular format — print-circle needed ==
(let ((l (list 1 2)))
  (setcdr (cdr l) l)
  (let ((print-circle t))
    (qa (format "%s" l) "#1=(1 2 . #1#)")))

;; == shared gensym requires print-circle to print #N= ==
;; remacs: print-circle doesn't dedup gensyms
(let* ((gs (make-symbol "test"))
       (l (list gs gs)))
  (let ((print-gensym t) (print-circle t))
    ;; "(#1=#:test #1#)" — "#1" sits at index 1, after the open paren
    (qa (string-match-p "#1" (prin1-to-string l)) 1))
  ;; without print-circle both print as separate #:test
  (let ((print-gensym t) (print-circle nil))
    (qa (string-match-p "#:test" (prin1-to-string l)) 1)))

;; == prin1-to-string on nil/t/keywords ==
(qa (prin1-to-string nil) "nil")
(qa (prin1-to-string t) "t")
(qa (prin1-to-string :key) ":key")

;; == dotted-pair print ==
(qa (prin1-to-string '(a . b)) "(a . b)")
(qa (prin1-to-string '(a b . c)) "(a b . c)")

;; == empty ==
(qa (prin1-to-string '()) "nil")
(qa (prin1-to-string "") "\"\"")

;; == symbol that looks like a number is escaped ==
;; remacs: prints '3\.5' as "3.5" without the backslash
(qa (prin1-to-string '3\.5) "\\3.5")
;; '1abc' reads as symbol — prints raw
(qa (prin1-to-string '1abc) "1abc")

;; == print-quoted nil doesn't affect plain strings ==
(let ((print-quoted nil))
  (qa (prin1-to-string "a") "\"a\""))

(princ (qa--done))
