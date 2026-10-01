;; `format' specifiers — GNU 31.1 -Q --batch ground truth.
;; Pinned semantics:
;;  * %s = princ, %S = prin1 (quotes strings); %d/%x/%o/%b truncate
;;    floats silently toward zero; %c requires characterp (wrong-type
;;    otherwise) and 0 prints as "".
;;  * Width `-'/`+'/`0'/`#'/precision flags work; # gives 0x/0/0b
;;    prefixes; *-width is NOT supported (error).
;;  * %e/%f/%g exist; %E/%G do NOT (invalid format operation).
;;  * %% = literal %; positional %N$ reordering IS supported.
;;  * Extra args are silently ignored; missing args / mismatched types /
;;    bad specifiers all signal plain `error' (not a subtype).
;;  * Format STRING must be a string (wrong-type-argument stringp).

;; == %s / %S ==
(qa (format "%s" "hi") "hi")
(qa (format "%S" "hi") "\"hi\"")
(qa (format "%s" 42) "42")
(qa (format "%S" 42) "42")
(qa (format "%s" 'sym) "sym")
(qa (format "%S" 'sym) "sym")
(qa (format "%s" 1.5) "1.5")
(qa (format "%s" nil) "nil")
(qa (format "%s" t) "t")
(qa (format "%s" '(a b)) "(a b)")
(qa (format "%S" '(a b)) "(a b)")
(qa (format "%S" [1 2]) "[1 2]")
(qa (format "%s" [1 2]) "[1 2]")
;; %s on a string drops its quoting but keeps the chars
(qa (format "%s" "a\nb") "a\nb")
;; %S does NOT escape newlines — embeds them literally
(qa (format "%S" "a\nb") "\"a\nb\"")
;; precision truncates %s output
(qa (format "%.3s" "abcdef") "abc")
(qa (format "%-10s|" "pad") "pad       |")
(qa (format "%10s|" "pad") "       pad|")

;; == %d ==
(qa (format "%d" 7) "7")
(qa (format "%d" -7) "-7")
(qa (format "%d" 3.9) "3")
(qa (format "%d" -3.9) "-3")
(qa (format "%d" ?A) "65")
(qa (format "%5d|%-5d|%+d|%05d" 3 3 3 3) "    3|3    |+3|00003")
(qa (format "%-+8d|" 42) "+42     |")
(qa (format "%05d" -42) "-0042")

;; == %x %o %b == (with # alternates; %b binary exists since 29ish)
(qa (format "%x" 255) "ff")
(qa (format "%X" 255) "FF")
(qa (format "%o" 8) "10")
(qa (format "%b" 5) "101")
(qa (format "%#x" 255) "0xff")
(qa (format "%#X" 255) "0XFF")
(qa (format "%#o" 8) "010")
(qa (format "%#b" 5) "0b101")
(qa (format "%x" -1) "-1")
(qa (format "%04x" 26) "001a")
(qa (format "%x" 1.5) "1")

;; == %c ==
(qa (format "%c" 65) "A")
(qa (format "%c" ?B) "B")
(qa (format "%5c" 65) "    A")
(qa (format "%c%c" 233 8364) "é€")
(qa (format "%c" 0) "\0")
(qa-err-data (format "%c" -1) 'wrong-type-argument '(characterp -1))
(qa-err (format "%c" "x") 'error)

;; == %e %f %g ==  (%E %G unsupported)
(qa (format "%f" 1.5) "1.500000")
(qa (format "%.2f" 1.2345) "1.23")
(qa (format "%8.2f" 1.2345) "    1.23")
(qa (format "%.0f" 2.5) "2")
(qa (format "%5.1f|x" 3.14159) "  3.1|x")
(qa (format "%08.3f" -1.5) "-001.500")
(qa (format "%f" 2) "2.000000")
(qa (format "%e" 12345.678) "1.234568e+04")
(qa (format "%.3e" 12345.678) "1.235e+04")
(qa (format "%5.2e" 0.00123) "1.23e-03")
(qa (format "%e" 0.0) "0.000000e+00")
(qa (format "%.0e" 500) "5e+02")
(qa (format "%g" 0.0001) "0.0001")
(qa (format "%g" 1e20) "1e+20")
(qa (format "%g" 100) "100")
(qa (format "%g" 1000000) "1e+06")
(qa (format "%.4g" 123456) "1.235e+05")
(qa (format "%.10g" 1.5) "1.5")

;; == %% ==
(qa (format "100%%") "100%")
(qa (format "%%%d%%" 5) "%5%")

;; == positional reordering %N$ ==
(qa (format "%1$s" "a") "a")
(qa (format "%2$s %1$s" "a" "b") "b a")

;; == format-messages style wrappers exist too ==
(qa (format-message "x%d" 3) "x3")
(qa (format "%s=%s" "a" "b") "a=b")

;; == errors — all plain `error' except stringp/characterp ==
(qa-err (format "%d" "x") 'error)
(qa-err (format "%s") 'error)
(qa-err (format "%s %s" "a") 'error)
(qa-err (format "%z" 3) 'error)
(qa-err (format "%G" 1.5) 'error)
(qa-err (format "%E" 1.5) 'error)
(qa-err (format "%*d" 5 3) 'error)
(qa-err (format "%f" "x") 'error)
(qa-err (format 5) 'wrong-type-argument)
(qa-err (format nil 1) 'wrong-type-argument)
;; error messages, pinned verbatim
(qa (condition-case e (format "%d" "x")
      (error (error-message-string e)))
    "Format specifier doesn’t match argument type")
(qa (condition-case e (format "%s")
      (error (error-message-string e)))
    "Not enough arguments for format string")
(qa (condition-case e (format "%z" 3)
      (error (error-message-string e)))
    "Invalid format operation %z")
;; extra args are ignored, NOT an error
(qa (format "x" 'extra-arg) "x")
(qa (format "%s" 5 :bad) "5")
;; no specifiers at all
(qa (format "") "")
(qa (format "abc") "abc")

;; == arity ==
(qa-err (format) 'wrong-number-of-arguments)

(princ (qa--done))
