;;; -*- lexical-binding: nil; -*-
;; GNU 31.1 verified; regression for #641 (nonterminating position parser).
(dolist (text '("80x24+bad" "80x24+1bad" "80x24+" "80x24+1-"
                "80x24+1+2+3" "80x24+999999999999999999bad"))
  (qa (x-parse-geometry text) nil))
(qa (x-parse-geometry "80x24+1-2")
    '((height . 24) (width . 80) (top . -2) (left . 1)))
(qa (x-parse-geometry "80x24+1")
    '((height . 24) (width . 80) (left . 1)))
(qa (x-parse-geometry "80x24+999999999999999999999-999999999999999999999")
    '((height . 24) (width . 80) (top . -2147483648) (left . 2147483647)))

;; Issue #3157: geometry sizes are unsigned ints, not signed ints.  GNU's
;; XParseGeometry clips larger values to UINT_MAX.
(qa (x-parse-geometry "2147483648x24")
    '((height . 24) (width . 2147483648)))
(qa (x-parse-geometry "4294967295x2147483648")
    '((height . 2147483648) (width . 4294967295)))
(qa (x-parse-geometry "4294967296x999999999999999999999999999999999999")
    '((height . 4294967295) (width . 4294967295)))

;; Issue #3145: the standard geometry spelling permits an optional `='
;; and upper-case `X' between width and height.
(qa (x-parse-geometry "=80x24+10+20")
    '((height . 24) (width . 80) (top . 20) (left . 10)))
(qa (x-parse-geometry "80X24+10+20")
    '((height . 24) (width . 80) (top . 20) (left . 10)))
(qa (x-parse-geometry "=80X24+10+20")
    '((height . 24) (width . 80) (top . 20) (left . 10)))

;; Issue #3155: a malformed size invalidates the whole geometry instead of
;; leaving a partial result containing only its valid offsets.
(dolist (text '("80x24x10+1+2" "garbage+1+2" "80x+1+2" "x+1+2"))
  (qa (x-parse-geometry text) nil))

;; Issue #3156: macOS's Nextstep spelling is four space-separated fields.
(when (eq system-type 'darwin)
  (qa (x-parse-geometry "100 200 24 80")
      '((top . 100) (left . 200) (height . 24) (width . 80)))
  (qa (x-parse-geometry "100 200")
      '((top . 100) (left . 200)))
  (qa (x-parse-geometry "100 200 24")
      '((top . 100) (left . 200) (height . 24))))

;; Issue #3147: a negative zero offset retains its right/bottom origin.
(qa (x-parse-geometry "-0-0") '((top - 0) (left - 0)))
(qa (x-parse-geometry "-0+0") '((top . 0) (left - 0)))
(qa (x-parse-geometry "+0-0") '((top - 0) (left . 0)))

(princ (qa--done))
