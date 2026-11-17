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

;; Numeric UTC offsets must be applied before converting to RESULT-ZONE (#2421).
(qa (icalendar--decode-isodatetime "20200101T120000Z" nil nil 32400)
    '(0 0 21 1 1 2020 3 nil 32400))
(qa (icalendar--decode-isodatetime "20200101T120000+0200" nil nil 32400)
    '(0 0 19 1 1 2020 3 nil 32400))
(qa (icalendar--decode-isodatetime "20200101T120000-0500" nil nil 32400)
    '(0 0 2 2 1 2020 4 nil 32400))

;; DATE-TIME fields must not roll over; second 60 is reserved for leap seconds.
(qa-err (icalendar--decode-isodatetime "20200101T240000Z") 'error)
(qa-err (icalendar--decode-isodatetime "20200101T126000Z") 'error)
(qa-err (icalendar--decode-isodatetime "20200101T120061Z") 'error)
(qa-err (icalendar--decode-isodatetime "20200101T120060Z") 'error)
(qa (condition-case nil
        (progn (icalendar--decode-isodatetime "19970630T235960Z") t)
      (error nil))
    t)

;; Component end dates must match DTSTART's value type and time form, and
;; must be strictly later (#2517, including the related VEVENT rule #2518).
(require 'icalendar-parser)
(require 'icalendar-utils)

(defun qa-icalendar-duration--component-valid-p (text)
  (with-temp-buffer
    (insert text)
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(defun qa-icalendar-duration--component (component properties)
  (format "BEGIN:%s\nUID:qa-end-order\nDTSTAMP:20250101T000000Z\n%s\nEND:%s"
          component (mapconcat #'identity properties "\n") component))

(defun qa-icalendar-duration--calendar-valid-p (method event-properties)
  (with-temp-buffer
    (insert "BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:-//qa//remacs//EN\n")
    (when method
      (insert "METHOD:PUBLISH\n"))
    (insert (qa-icalendar-duration--component "VEVENT" event-properties)
            "\nEND:VCALENDAR")
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART:20250102T090000Z" "DUE:20250102T100000Z")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART:20250102T090000" "DUE:20250102T100000")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART:20250102T090000Z" "DUE:20250101T090000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART:20250102T090000Z" "DUE:20250102T090000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART:20250102T090000" "DUE:20250102T100000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART;VALUE=DATE:20250102"
                "DUE:20250103T090000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART;VALUE=DATE:20250102"
                "DUE;VALUE=DATE:20250103")))
    t)

;; A DATE DTSTART in VTODO also limits DURATION to whole days or weeks (#2516).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART;VALUE=DATE:20250102" "DURATION:PT1H")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("DTSTART;VALUE=DATE:20250102" "DURATION:P1D")))
    t)

(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250102T090000Z" "DTEND:20250102T100000Z")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250102T090000" "DTEND:20250102T100000")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250102T090000Z" "DTEND:20250101T090000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250102T090000" "DTEND:20250102T100000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART;VALUE=DATE:20250102"
                 "DTEND:20250103T090000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART;VALUE=DATE:20250102"
                 "DTEND;VALUE=DATE:20250103")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART;VALUE=DATE:20250102" "DURATION:PT1H")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART;VALUE=DATE:20250102" "DURATION:P1D")))
    t)

;; VFREEBUSY timestamps and every explicit FREEBUSY PERIOD endpoint are UTC
;; (#2519, including the related ordering and endpoint reports #2520/#2521).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("DTSTART:20250101T090000"
                    "DTEND:20250101T100000")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("DTSTART;VALUE=DATE:20250101")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("DTSTART:20250101T090000Z"
                    "DTEND:20250101T100000Z")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("DTSTART:20250102T090000Z"
                    "DTEND:20250101T090000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("FREEBUSY:20250101T090000/20250101T100000")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("FREEBUSY:20250101T090000Z/20250101T100000Z")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("FREEBUSY:20250101T090000Z/20250103T100000")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("FREEBUSY:20250101T090000/20250103T100000Z")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("FREEBUSY:20250101T090000Z/P1D")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VFREEBUSY" '("FREEBUSY:20250101T090000/P1D")))
    nil)

;; A VEVENT needs DTSTART unless its containing calendar specifies METHOD (#2522).
(qa (qa-icalendar-duration--calendar-valid-p
     nil '("SUMMARY:Missing start"))
    nil)
(qa (qa-icalendar-duration--calendar-valid-p
     t '("SUMMARY:Method allows missing start"))
    t)
(qa (qa-icalendar-duration--calendar-valid-p
     nil '("DTSTART:20250101T120000Z"))
    t)

(princ (qa--done))
