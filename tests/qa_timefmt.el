;; timefmt — encode-time/decode-time/format-time-string pinned at fixed
;; UTC timestamps, time arithmetic, format-spec, and format edge cases.
;; Locale-sensitive names (%A %B %p %c %x %X %r %h) are pinned under
;; (let ((system-time-locale "C")) ...) — the ambient locale may differ.
;; `decoded-time-p' does not exist in 31.1 (void-function); skipped.
(require 'format-spec)

;; == encode-time ==
;; ZONE t, "UTC" and 0 all mean UTC; DST -1 means "guess".
(qa (encode-time '(0 0 0 1 1 2000 nil -1 t)) '(14445 17280))
(qa (encode-time '(0 0 0 1 1 2000 nil -1 "UTC")) '(14445 17280))
(qa (encode-time '(0 0 0 1 1 2000 nil -1 0)) '(14445 17280))
(qa (encode-time '(59 59 23 31 12 1999 nil -1 t)) '(14445 17279))
(qa (encode-time '(0 0 0 29 2 2000 nil -1 t)) '(14523 3072))
;; DST flag is irrelevant when ZONE is UTC.
(qa (encode-time '(0 0 0 1 1 2000 nil t t)) '(14445 17280))
;; Out-of-range fields normalize: Jan 32 -> Feb 1, hour 24 -> next day.
(qa (format-time-string "%Y-%m-%d"
        (encode-time '(0 0 0 32 1 2000 nil -1 t)) t)
    "2000-02-01")
(qa (format-time-string "%Y-%m-%d %H:%M"
        (encode-time '(0 0 24 1 1 2000 nil -1 t)) t)
    "2000-01-02 00:00")
;; Sub-second SEC: (HIGH LOW) and (TICKS . HZ) forms both accepted.
(qa (encode-time '((0 0) 15 14 15 6 2020 nil -1 t)) '(24295 33380))
(qa (encode-time '((1000 . 1) 15 14 15 6 2020 nil -1 t)) '(24295 34380))
;; Obsolete multi-argument convention still works in 31.1.
(qa (encode-time 0 0 0 1 1 2000 nil -1 t) '(14445 17280))
;; Explicit numeric zone, decoded back under the same zone.
(qa (decode-time (encode-time '(0 0 0 15 6 2020 nil -1 3600)) 3600)
    '(0 0 0 15 6 2020 1 nil 3600))
(qa (decode-time (encode-time '(0 0 0 15 6 2020 nil -1 -14400)) -14400)
    '(0 0 0 15 6 2020 1 nil -14400))
(qa-err-data (encode-time 5) 'wrong-type-argument '(consp 5))
(qa-err-data (encode-time 946684800) 'wrong-type-argument '(consp 946684800))

;; == decode-time ==
(qa (decode-time '(0 0) t) '(0 0 0 1 1 1970 4 nil 0))
(qa (decode-time 1592230530 t) '(30 15 14 15 6 2020 1 nil 0))
(qa (decode-time 1592230530 "UTC") '(30 15 14 15 6 2020 1 nil 0))
(qa (decode-time 1592230530 0) '(30 15 14 15 6 2020 1 nil 0))
;; Negative times decode before the epoch.
(qa (decode-time -86400 t) '(0 0 0 31 12 1969 3 nil 0))
;; The DOW slot in encode-time input is ignored; it is output-only.
(qa (decode-time (encode-time '(0 0 0 15 6 2020 1 -1 t)) t)
    '(0 0 0 15 6 2020 1 nil 0))
(qa-err-data (decode-time "x") 'error '("Invalid time specification"))
(qa-err-data (decode-time 0 'bogus)
             'error '("Invalid time zone specification" bogus))

;; == format-time-string: locale-free specs ==
;; t0 = epoch, t1 = 2020-06-15 14:15:30 UTC, t2 = 2000-01-01 UTC.
(qa (format-time-string "%Y-%m-%d" '(0 0) t) "1970-01-01")
(qa (format-time-string "%H:%M:%S" '(0 0) t) "00:00:00")
(qa (format-time-string "%s" '(0 0) t) "0")
(qa (format-time-string "%Z" '(0 0) t) "UTC")
(qa (format-time-string "%z" '(0 0) t) "+0000")
(qa (format-time-string "%s" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "1592230530")
(qa (format-time-string "%F %T" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "2020-06-15 14:15:30")
(qa (format-time-string "%D %R" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "06/15/20 14:15")
(qa (format-time-string "%y %C" (encode-time '(0 0 0 1 1 2000 nil -1 t)) t)
    "00 20")
;; %g is the last two digits of the ISO week-numbering year %G:
;; 2000-01-01 is in ISO week 1999-W52, so %G=1999 %g=99.
(qa (format-time-string "%G %g" (encode-time '(0 0 0 1 1 2000 nil -1 t)) t)
    "1999 99")
(qa (format-time-string "%j" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "167")
(qa (format-time-string "%j" (encode-time '(0 0 0 31 12 2000 nil -1 t)) t)
    "366")
;; Week numbers: %U (Sunday start) and %W (Monday start) agree here;
;; %V/%G/%u are ISO week, year, weekday (Monday=1); %w Sunday=0.
(qa (format-time-string "%U %W" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "24 24")
(qa (format-time-string "%V %G %u %w"
        (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "25 2020 1 1")
;; ISO week-year boundaries: Dec 30 2019 is ISO 2020-W01-1, while
;; Jan 1 2021 is still ISO 2020-W53.
(qa (format-time-string "%Y %G %V %u"
        (encode-time '(0 0 0 30 12 2019 nil -1 t)) t)
    "2019 2020 01 1")
(qa (format-time-string "%Y %G %V"
        (encode-time '(0 0 0 1 1 2021 nil -1 t)) t)
    "2021 2020 53")
(qa (format-time-string "%Y %U %W"
        (encode-time '(0 0 0 1 1 2021 nil -1 t)) t)
    "2021 00 00")
(qa (format-time-string "%I %l %k %M %S"
        (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "02  2 14 15 30")
(qa (format-time-string "%m %d %e" (encode-time '(0 0 0 5 6 2020 nil -1 t)) t)
    "06 05  5")
;; Padding modifiers: %- strips, %_ space-pads, %0 zero-pads.
(qa (format-time-string "%-d|%_d|%0d" (encode-time '(0 0 0 5 6 2020 nil -1 t)) t)
    "5| 5|05")
(qa (format-time-string "%-m" (encode-time '(0 0 0 5 6 2020 nil -1 t)) t) "6")
;; %q is quarter of year (not in strftime; GNU extension).
(qa (format-time-string "%q" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "2")
(qa (format-time-string "%q" (encode-time '(0 0 0 1 1 2000 nil -1 t)) t)
    "1")
;; %n and %t emit newline and tab.
(qa (format-time-string "a%nb%tc" '(0 0) t) "a\nb\tc")
(qa (format-time-string "%%" '(0 0) t) "%")
;; %N nanoseconds, default and width-limited; MICRO/PICO slots fold in.
(qa (format-time-string "%N" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "000000000")
(qa (format-time-string "%3N" (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "000")
(qa (format-time-string "%N" '(24295 33410 500000) t) "500000000")
(qa (format-time-string "%6N" '(24295 33410 500000) t) "500000")
(qa (format-time-string "%N" '(24295 33410 500000 123000) t) "500000123")
;; %:z / %::z / %:::z colon-style zone offsets.
(qa (format-time-string "%:z %::z %:::z"
        (encode-time '(30 15 14 15 6 2020 nil -1 t)) t)
    "+00:00 +00:00:00 +00")
(qa (format-time-string "%z %:z %::z"
        (encode-time '(30 15 14 15 6 2020 nil -1 t)) -14400)
    "-0400 -04:00 -04:00:00")
;; %Z under a numeric zone prints the signed-hour abbreviation.
(qa (format-time-string "%Z" (encode-time '(30 15 14 15 6 2020 nil -1 t)) 3600)
    "+01")
;; Unknown specs pass through literally (%E/%O modifiers unsupported).
(qa (format-time-string "%Q" '(0 0) t) "%Q")
(qa (format-time-string "%E %O" '(0 0) t) "%E %O")
;; Time value forms: integer seconds, (TICKS . HZ) cons, (HIGH LOW) list.
(qa (format-time-string "%Y-%m-%d" 0 t) "1970-01-01")
(qa (format-time-string "%Y" -1 t) "1969")
(qa (format-time-string "%Y-%m-%d %H:%M:%S" -1 t) "1969-12-31 23:59:59")
(qa (format-time-string "%Y-%m-%d" '(0 . 1) t) "1970-01-01")
(qa (format-time-string "%Y-%m-%d" '(86400 . 1) t) "1970-01-02")
(qa (format-time-string "%Y-%m-%d" '(65535 65535) t) "2106-02-07")
(qa (format-time-string "%F %T" (encode-time '(0 0 12 29 2 2024 nil -1 t)) t)
    "2024-02-29 12:00:00")
(qa-err-data (format-time-string "%Y" "x" t)
             'error '("Invalid time specification"))
(qa-err-data (format-time-string "%Y" '(0 0) 'bogus)
             'error '("Invalid time zone specification" bogus))

;; == format-time-string: locale-sensitive specs under C locale ==
(let ((system-time-locale "C"))
  (let ((t1 (encode-time '(30 15 14 15 6 2020 nil -1 t))))
    (qa (format-time-string "%A %a" t1 t) "Monday Mon")
    (qa (format-time-string "%B %b %h" t1 t) "June Jun Jun")
    (qa (format-time-string "%p %P" t1 t) "PM pm")
    (qa (format-time-string "%c" t1 t) "Mon Jun 15 14:15:30 2020")
    (qa (format-time-string "%x %X" t1 t) "06/15/20 14:15:30")
    (qa (format-time-string "%r" t1 t) "02:15:30 PM"))
  (qa (format-time-string "%p" '(0 0) t) "AM")
  (qa (format-time-string "%A" (encode-time '(0 0 0 2 1 2000 nil -1 t)) t)
      "Sunday"))

;; == time conversion and arithmetic ==
(qa (time-convert 0 'list) '(0 0 0 0))
(qa (time-convert '(0 . 1) 'integer) 0)
(qa (time-convert '(24295 33410) 'integer) 1592230530)
(qa (time-convert '(24295 33410) 'list) '(24295 33410 0 0))
(qa (time-convert 1592230530 'list) '(24295 33410 0 0))
(qa (time-convert 1592230530 1) '(1592230530 . 1))
;; (TICKS . HZ) cons times.
(qa (time-convert '(100 . 1000) 'list) '(0 0 100000 0))
(qa (time-convert '(100 . 1000) 'integer) 0)
(qa (time-convert '(1500 . 1000) 'integer) 1)
;; Non-default FREQUENCY yields a (TICKS . HZ) pair, possibly bignum.
(qa (time-convert '(24295 33410) 1000) '(1592230530000 . 1000))
(qa (time-convert '(24295 33410 500000 123456) 1000000000)
    '(1592230530500000123 . 1000000000))
(qa (time-convert '(24295 33410 500000 123456) 'integer) 1592230530)
(qa-err-data (time-convert 0 'bogus)
             'error '("Invalid time frequency" bogus))

(qa (time-add 1592230530 1000) 1592231530)
(qa (time-add 1 2) 3)
(qa (time-add '(1592230530 . 1000) '(500 . 1000)) '(1592231030 . 1000))
(qa (time-add '(24295 33410 0 0) 1.5) '(24295 33411 500000 0))
;; time-subtract quirk: eq operands return the 4-list zero, merely
;; equal operands return integer 0.
(let ((qa-t (list 24295 33410)))
  (qa (time-subtract qa-t qa-t) '(0 0 0 0)))
(qa (time-subtract '(24295 33410) '(24295 33410)) 0)
(qa (time-subtract '(24295 33410) '(24295 33409)) 1)
(qa (time-subtract '(1592230530 . 1000) '(500 . 1000)) '(1592230030 . 1000))
;; Borrow across the denominator.
(qa (time-subtract '(1592230530 . 1000) '(700 . 1000)) '(1592229830 . 1000))
(qa-err-data (time-subtract '(24295 33410) '(1000))
             'error '("Invalid time specification"))
(qa-err-data (time-add 1 'hz) 'error '("Invalid time specification"))

(qa (time-less-p '(0 0) '(24295 33410)) t)
(qa (time-less-p '(24295 33410) '(24295 33410)) nil)
(qa (time-less-p '(1 . 2) '(2 . 1)) t)
(qa (time-equal-p '(24295 33410) '(24295 33410)) t)
;; Comparison works across different time representations.
(qa (time-equal-p 1592230530 '(24295 33410)) t)
(qa (time-equal-p '(1 . 1) 1) t)

(qa (float-time '(0 0)) 0.0)
(qa (float-time '(0 . 1)) 0.0)
(qa (float-time 1500) 1500.0)
(qa (float-time '(24295 33410)) 1592230530.0)
(qa (float-time '(100 . 4)) 25.0)
(qa (float-time '(24295 33410 500000)) 1592230530.5)

(qa (seconds-to-time 0) '(0 0 0 0))
(qa (seconds-to-time 946684800) '(14445 17280 0 0))
(qa (seconds-to-time -1) '(-1 65535 0 0))
(qa (seconds-to-time 1.5) '(0 1 500000 0))

(qa (time-to-days 0) 719163)
(qa (time-to-days '(14445 17280)) 730120)
(qa (time-to-days -86400) 719162)
(qa (time-to-day-in-year '(14445 17280)) 1)
(qa (time-to-day-in-year '(24295 33410)) 167)

;; current-time shape only; the value itself is nondeterministic.
(qa (consp (current-time)) t)
(qa (length (current-time)) 4)
(qa (stringp (current-time-string '(0 0) t)) t)

;; == decoded-time-* accessors ==
(let ((d '(30 15 14 15 6 2020 1 nil 0)))
  (qa (decoded-time-second d) 30)
  (qa (decoded-time-minute d) 15)
  (qa (decoded-time-hour d) 14)
  (qa (decoded-time-day d) 15)
  (qa (decoded-time-month d) 6)
  (qa (decoded-time-year d) 2020)
  (qa (decoded-time-weekday d) 1)
  (qa (decoded-time-dst d) nil)
  (qa (decoded-time-zone d) 0))

;; == format-spec ==
(qa (format-spec "%a %b" '((?a . "x") (?b . 2))) "x 2")
(qa (format-spec "%s" '((?s . "hi"))) "hi")
(qa (format-spec "%%" '((?a . "x"))) "%")
(qa (format-spec "%a%%%b" '((?a . "L") (?b . "R"))) "L%R")
(qa (format-spec "%a%b" '((?a . "A") (?b . "B"))) "AB")
(qa (format-spec "%a %a" '((?a . "v"))) "v v")
(qa (format-spec "no specs" '((?a . "x"))) "no specs")
;; First duplicate alist entry wins.
(qa (format-spec "%a" '((?a . "x") (?a . "y"))) "x")
;; Non-string values render via %s: chars become their code number.
(qa (format-spec "%a" '((?a . 3.5))) "3.5")
(qa (format-spec "%a" '((?a . sym))) "sym")
(qa (format-spec "%a" '((?a . ?X))) "88")
(qa (format-spec "%a" '((97 1 2))) "(1 2)")
(qa (format-spec "%a" '((?a . ""))) "")
;; Width/precision/flags on specs.
(qa (format-spec "%10a|" '((?a . "x"))) "         x|")
(qa (format-spec "%-10a|" '((?a . "x"))) "x         |")
(qa (format-spec "%010a" '((?a . 5))) "0000000005")
(qa (format-spec "%.3a" '((?a . "abcdef"))) "abc")
(qa (format-spec "%4.2a" '((?a . "abcdef"))) "  ab")
(qa (format-spec "%-4a|%.2a" '((?a . "xyz"))) "xyz |xy")
;; Precision truncates the rendered string, it does not round numbers.
(qa (format-spec "%08.3f" '((?f . 3.14159))) "000003.1")
;; A digit is width, not positional.
(qa (format-spec "%1a" '((?a . "z"))) "z")
;; Spec letters are case-sensitive; unknown or missing keys error.
(qa-err (format-spec "%z" '((?a . "x"))) 'error)
(qa-err (format-spec "%A" '((?a . "x"))) 'error)
(qa-err (format-spec "%a" nil) 'error)
(qa-err (format-spec "%b" '((?a . "x"))) 'error)
;; A nil-valued entry counts as missing.
(qa-err (format-spec "%a" '((?a))) 'error)
;; IGNORE-ARGS: a list of spec chars leaves those specs untouched.
(qa (format-spec "%b%a%b" '((?a . "X")) '(?b)) "%bX%b")
;; Non-list IGNORE-ARGS also leaves unknown specs literal.
(qa (format-spec "%a%z" '((?a . "v")) 'ignore) "v%z")
(qa (format-spec "%a%z" '((?a . "v")) 'delete) "v")
;; format-spec-make builds the alist.
(qa (format-spec "%a %b" (format-spec-make ?a "v1" ?b "v2")) "v1 v2")
(qa-err (format-spec "%a" (format-spec-make ?a)) 'error)
;; Value text containing %-sequences is not re-expanded.
(qa (format-spec "%a" '((?a . "p%bq"))) "p%bq")

;; == format extras ==
;; %d and %x silently truncate floats.
(qa (format "%d" 1.9) "1")
(qa (format "%d" -1.9) "-1")
(qa (format "%d" 7.0) "7")
(qa (format "%x" 1.5) "1")
(qa (format "%d" 1e18) "1000000000000000000")
;; Negative hex/octal print a leading minus, not two's-complement.
(qa (format "%x %o" -1 -1) "-1 -1")
(qa (format "%x" -255) "-ff")
(qa (format "%#x %#o" 255 8) "0xff 010")
(qa (format "%x %o" 255 64) "ff 100")
;; %s prints a character argument as its integer code.
(qa (format "%s %d %x" ?x ?x ?a) "120 120 61")
;; %c inserts the character; range is checked with characterp.
(qa (format "%c" 65) "A")
(qa-err-data (format "%c" -1) 'wrong-type-argument '(characterp -1))
(qa-err-data (format "%c" 4194304) 'wrong-type-argument '(characterp 4194304))
;; Precision truncates strings; %- left-pads... er, right-pads.
(qa (format "%.3s" "abcdef") "abc")
(qa (format "%5.2s|" "abcdef") "   ab|")
(qa (format "%-5s|" "ab") "ab   |")
(qa (format "%-10.4s|" "abcdefg") "abcd      |")
(qa (format "%5c|" ?a) "    a|")
(qa (format "%05d %+d % d" 42 42 42) "00042 +42  42")
(qa (format "%.0d" 0) "")
(qa (format "%.0d" 5) "5")
;; Float specs.
(qa (format "%s %s %s" 1.0 1.5e300 1e20) "1.0 1.5e+300 1e+20")
(qa (format "%s %s %s" 1.0e+INF -1.0e+INF 0.0e+NaN)
    "1.0e+INF -1.0e+INF 0.0e+NaN")
(qa (format "%.10f" 0.1) "0.1000000000")
;; %f rounds half-to-even.
(qa (format "%.0f %.0f" 2.5 3.5) "2 4")
(qa (format "%e %g %g" 12345.678 0.0001 1e20)
    "1.234568e+04 0.0001 1e+20")
(qa (format "%.17g" 0.1) "0.10000000000000001")
(qa (format "%7.3f|" 3.14159) "  3.142|")
(qa (format "%08.3f" 3.14159) "0003.142")
;; %N$ selects argument N (1-based); reuse and reorder are allowed.
(qa (format "%1$s %2$s" "a" "b") "a b")
(qa (format "%1$s %1$s" "a" "b") "a a")
(qa (format "%2$s-%1$s" "a" "b") "b-a")
(qa-err (format "%3$s" "a" "b") 'error)
;; Extra arguments are silently ignored.
(qa (format "%s %s" "a" "b" "c") "a b")
(qa (format "abc" 1 2) "abc")
;; Literal %% consumes no argument.
(qa (format "%%s" "x") "%s")
(qa (format "%%%%") "%%")
;; %S prin1-quotes; %s on lists/vectors prints like prin1 anyway.
(qa (format "%S" "a\"b") "\"a\\\"b\"")
(qa (format "%S" "a\tb") "\"a\tb\"")
(qa (format "%s" '(1 2)) "(1 2)")
(qa (format "%S" '(1 2)) "(1 2)")
(qa (format "%s" [1 2]) "[1 2]")
(qa (format "%S" 'foo\ bar) "foo\\ bar")
(qa (format "%S" :kw) ":kw")
(qa (format "%s %s" nil t) "nil t")
;; String arguments are inserted verbatim, % included.
(qa (format "%s" "nested %d %") "nested %d %")
;; Text properties survive into the formatted result.
(qa (get-text-property 0 'x (format "%s" (propertize "a" 'x 1))) 1)
;; Properties of the format string itself are copied onto the output.
(qa (get-text-property 0 'x (format (propertize "%s" 'x 1) "y")) 1)
;; Errors: type mismatch, missing args, bad specs.
(qa-err (format "%d" "x") 'error)
(qa-err (format "%s %s" "only") 'error)
(qa-err (format "%s") 'error)
(qa-err (format "%z" 1) 'error)
(qa-err (format "%*s" 5 "x") 'error)
(qa-err (format "%" 1) 'error)
;; format-message exists and matches format on plain specs.
(qa (format-message "%s %s" "a" "b") "a b")

(princ (qa--done))
