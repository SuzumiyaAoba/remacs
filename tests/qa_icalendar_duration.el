;;; qa_icalendar_duration.el --- iCalendar duration regression checks -*- lexical-binding: t; -*-

(require 'icalendar)

;; RFC 5545 section 3.3.6 requires a nonempty duration, ordered components,
;; and a sign before P.  GNU Emacs 31.1 accepts several malformed values
;; reported in #2416 and its duplicate reports.
(qa (icalendar--decode-isoduration "P1D") '(0 0 0 1 0 0))
(qa (icalendar--decode-isoduration "P1W") '(0 0 0 7 0 0))
(qa (icalendar--decode-isoduration "+P1D") '(0 0 0 1 0 0))
(qa (icalendar--decode-isoduration "-P1DT2H3M4S") '(-4 -3 -2 -1 0 0))
(qa (icalendar--decode-isoduration "PT3S") '(3 0 0 0 0 0))
(qa (icalendar--decode-isoduration "PT2H3M4S") '(4 3 2 0 0 0))
(qa (icalendar--decode-isoduration "PT2h3m4s") '(4 3 2 0 0 0))
(qa (icalendar--decode-isoduration "P2D" t) '(0 0 0 1 0 0))
(qa (icalendar--decode-isoduration nil) nil)

(qa-err (icalendar--decode-isoduration "P") 'error)
(qa-err (icalendar--decode-isoduration "PT") 'error)
(qa-err (icalendar--decode-isoduration "P1DT") 'error)
(qa-err (icalendar--decode-isoduration "P-1D") 'error)
(qa-err (icalendar--decode-isoduration "PT2H3S") 'error)
(qa-err (icalendar--decode-isoduration "PT3M2H") 'error)
(qa-err (icalendar--decode-isoduration "P1W2D") 'error)

(princ (qa--done))
