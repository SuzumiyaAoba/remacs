;; probe_printread — printer/reader branch coverage
;; --- printer: all value kinds ---
(dolist (v '(nil t 42 -7 3.14 -0.5 1e10 1.5e-3 "str" "es\"c" "nl\n" "tab\t"
             sym |weird\ sym| ?a ?\n ?\C-a ?\M-x [1 2 3] (a b c) (a . b)
             #&4"1010" #'car 'car))
  (ignore-errors (prin1-to-string v))
  (ignore-errors (princ-to-string v)))
(ignore-errors (prin1-to-string (make-marker)))
(ignore-errors (prin1-to-string (point-marker)))
(ignore-errors (prin1-to-string (current-buffer)))
(ignore-errors (prin1-to-string (selected-window)))
(ignore-errors (prin1-to-string (selected-frame)))
(ignore-errors (prin1-to-string (make-char-table 'x)))
(ignore-errors (prin1-to-string (make-hash-table)))
(ignore-errors (prin1-to-string (lambda (x) x)))
(ignore-errors (prin1-to-string (function (lambda (x y &optional z) x))))
(ignore-errors (prin1-to-string (syntax-table)))
(ignore-errors (prin1-to-string (make-sparse-keymap)))
(ignore-errors (prin1-to-string (current-global-map)))
;; print vars
(let ((print-length 3))
  (ignore-errors (prin1-to-string '(1 2 3 4 5)))
  (ignore-errors (prin1-to-string [1 2 3 4 5])))
(let ((print-length 0))
  (ignore-errors (prin1-to-string '(1 2 3)))
  (ignore-errors (prin1-to-string [1 2])))
(let ((print-level 2))
  (ignore-errors (prin1-to-string '(1 (2 (3 (4))))))
  (ignore-errors (prin1-to-string [[1 2] [3 4]])))
(let ((print-escape-newlines nil))
  (ignore-errors (prin1-to-string "a\nb")))
(let ((print-escape-newlines t))
  (ignore-errors (prin1-to-string "a\nb")))
(let ((print-escape-nonascii t))
  (ignore-errors (prin1-to-string "é")))
(let ((print-escape-multibyte nil))
  (ignore-errors (prin1-to-string "é")))
(let ((print-circle t))
  (ignore-errors (prin1-to-string '(1 2 3))))
(let ((print-gensym t))
  (ignore-errors (prin1-to-string (make-symbol "g"))))
(let ((print-gensym nil))
  (ignore-errors (prin1-to-string (make-symbol "g"))))
(let ((print-quoted nil))
  (ignore-errors (prin1-to-string '(quote x))))
(let ((print-quoted t))
  (ignore-errors (prin1-to-string '(quote x))
    (prin1-to-string '(function f))
    (prin1-to-string '`(a ,b ,@c))))
;; symbol printing with escapes
(ignore-errors (prin1-to-string (intern "a b")))
(ignore-errors (prin1-to-string (intern "")))
(ignore-errors (prin1-to-string (intern "123")))
(ignore-errors (prin1-to-string (intern "+")))
(ignore-errors (prin1-to-string (intern ".")))
(ignore-errors (prin1-to-string (intern "a.b")))
(ignore-errors (prin1-to-string (intern "A#B")))
(ignore-errors (prin1-to-string (intern "sym:colon")))
(ignore-errors (prin1-to-string (intern "UPPER")))
(ignore-errors (princ-to-string (intern "a b")))
;; char printing
(ignore-errors (prin1-to-string ?\ ))
(ignore-errors (prin1-to-string ?\C- ))
(ignore-errors (prin1-to-string 127))
(ignore-errors (prin1-to-string ?\M-\C-a))
(ignore-errors (prin1-to-string ?\S-a))
(ignore-errors (prin1-to-string ?é))
;; format coverage
(ignore-errors (format "%s %S %d %c %e %f %g %%" 'x "s" 1 ?a 1.5 1.5 1.5))
(ignore-errors (format "%5d|%-5d|%05d" 3 3 3))
(ignore-errors (format "%10s|%-10s|%.2s" "abc" "abc" "abc"))
(ignore-errors (format "%.3f|%.1e|%.3g" 3.14159 3141.59 314159.0))
(ignore-errors (format "%x|%o|%d" 255 8 8))
(ignore-errors (format "%c" 65))
(ignore-errors (format "%s" nil))
(ignore-errors (format "%S" nil))
(ignore-errors (format "literal"))
(ignore-errors (format "%s" 1 2 3))
(ignore-errors (format "%*d|%.*f|%*s" 5 3 2 3.14159 8 "x"))
(ignore-errors (format-message "a %s" 'b))
(ignore-errors (pp '(1 2 (3 4) 5)))
(ignore-errors (pp-to-string '(1 2 (3 4))))
;; output functions
(ignore-errors (terpri nil))
(ignore-errors (print 42))
(ignore-errors (print "s"))
(ignore-errors (print [1]))
(ignore-errors (print '(a . b)))
(ignore-errors (prin1 42))
(ignore-errors (princ 42))
(ignore-errors (princ "s"))
(ignore-errors (write-char ?a))
(ignore-errors (with-output-to-string (princ "x") (princ 42)))
(ignore-errors (with-temp-buffer (princ "x" (current-buffer)) (buffer-string)))
(let ((standard-output (current-buffer)))
  (ignore-errors (princ "y")))
;; --- reader: literal syntax ---
(dolist (s '("42" "-7" "+9" "3.25" "-0.5" "1e3" "1.5e-2" "1E+3" ".5" "5."
             "#x1F" "#Xff" "#o17" "#b101" "#16r1f" "#8r17" "#2r11"
             "foo" "+foo" "-foo" "foo-bar" "1+"
             "'(1 2)" "`(a ,b ,@c)" "#'f" "#'(lambda (x) x)"
             "\"str\"" "\"e\\n\\t\\r\\\\\\\"s\""
             "\"\\x41\\101\\u0041\"" "\"\\^a\" \"\\C-a\" \"\\M-a\" \"\\S-a\""
             "\"\\a\\b\\f\\v\" \"\\s\" \"\\d\""
             "?a" "?A" "?\\n" "?\\t" "?\\s" "?\\\\" "?\\\"" "?\\(" "?\\)"
             "?\\C-a" "?\\C-M-a" "?\\M-a" "?\\S-a" "?\\^a" "?\\^I"
             "?\\x41" "?\\u00e9" "?\\N{LATIN SMALL LETTER A}"
             "[1 2 3]" "[]" "[\"a\" 'b 3.5]"
             "#&8\"\\377\"" "#&4\"\\x0f\"" "#&0\"\""
             "#s(rec 1 2)"
             "#(\"str\" 0 3 (face bold))"
             "#1=(a . #1#)"
             "; comment\n42"
             "#| block\ncomment |# 42"
             "(a . b)" "(a . (b))" "(a b . c)"
             "()" "( )" "(a (b (c)))"
             "t" "nil"))
  (ignore-errors (read-from-string s)))
;; reader via read/eval
(ignore-errors (read "(+ 1 2)"))
(ignore-errors (read "\"s\""))
(ignore-errors (car (read-from-string "(a b)")))
(ignore-errors (read-from-string "(a b) trailing"))
(ignore-errors (read-from-string "  42  "))
(ignore-errors (read-from-string ""))
;; unread / streams
(ignore-errors (with-temp-buffer
                 (insert "(1 2) rest")
                 (goto-char (point-min))
                 (list (read (current-buffer)) (point))))
(ignore-errors (with-temp-buffer
                 (insert "(1 2)")
                 (goto-char (point-min))
                 (read (current-buffer))
                 (unread-command-events)))
;; malformed input → error paths
(dolist (s '("(" ")" "(a" "[1" "\"unterm" "#(" "#<" "#z" "?" "?\\z" ",x" "#'"))
  (ignore-errors (read-from-string s)))
