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

(defun qa-icalendar-duration--calendar-content-valid-p (method content)
  (with-temp-buffer
    (insert "BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:-//qa//remacs//EN\n")
    (when method
      (insert "METHOD:PUBLISH\n"))
    (insert content "\nEND:VCALENDAR")
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(defun qa-icalendar-duration--calendar-component-valid-p
    (method component properties)
  (qa-icalendar-duration--calendar-content-valid-p
   method (qa-icalendar-duration--component component properties)))

(defun qa-icalendar-duration--calendar-valid-p (method event-properties)
  (qa-icalendar-duration--calendar-component-valid-p
   method "VEVENT" event-properties))

(defun qa-icalendar-duration--geo-valid-p (text)
  (condition-case nil
      (progn
        (icalendar-parse-from-string 'icalendar-geo-coordinates text)
        t)
    (error nil)))

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

;; VCALENDAR cannot define a TZID more than once (#2525).
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/Duplicate\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0000\nTZOFFSETTO:+0000\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VTIMEZONE\nTZID:Test/Duplicate\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0100\nTZOFFSETTO:+0100\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VEVENT\nUID:duplicate-zone\n"
          "DTSTAMP:20250101T000000Z\n"
          "DTSTART;TZID=Test/Duplicate:20250101T090000\n"
          "END:VEVENT"))
    nil)
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/One\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0000\nTZOFFSETTO:+0000\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VTIMEZONE\nTZID:Test/Two\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0100\nTZOFFSETTO:+0100\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VEVENT\nUID:distinct-zones\n"
          "DTSTAMP:20250101T000000Z\n"
          "DTSTART;TZID=Test/One:20250101T090000\n"
          "END:VEVENT"))
    t)

;; Numeric iCalendar properties enforce their inclusive RFC ranges (#2526/#2527).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VTODO" '("PERCENT-COMPLETE:-1")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VTODO" '("PERCENT-COMPLETE:101")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VTODO" '("PERCENT-COMPLETE:0")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VTODO" '("PERCENT-COMPLETE:100")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("PRIORITY:-1")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("PRIORITY:10")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("PRIORITY:0")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("PRIORITY:9")))
    t)

;; GEO coordinates stay within latitude/longitude limits (#2528).
(qa (qa-icalendar-duration--geo-valid-p "91.0;0.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "-91.0;0.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "0.0;181.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "0.0;-181.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "90.0;180.0") t)
(qa (qa-icalendar-duration--geo-valid-p "-90.0;-180.0") t)

;; Relative VALARM triggers require the corresponding parent start or end
;; time, even when METHOD otherwise permits a VEVENT without DTSTART (#2523/#2524).
(qa (qa-icalendar-duration--calendar-component-valid-p
     t "VEVENT" '("BEGIN:VALARM" "ACTION:DISPLAY" "TRIGGER:-PT15M"
                  "DESCRIPTION:Reminder" "END:VALARM"))
    nil)
(qa (qa-icalendar-duration--calendar-component-valid-p
     t "VEVENT" '("DTSTART:20250101T120000Z"
                  "BEGIN:VALARM" "ACTION:DISPLAY" "TRIGGER:-PT15M"
                  "DESCRIPTION:Reminder" "END:VALARM"))
    t)
(qa (qa-icalendar-duration--calendar-component-valid-p
     t "VEVENT" '("DTSTART:20250101T120000Z"
                  "BEGIN:VALARM" "ACTION:DISPLAY"
                  "TRIGGER;RELATED=END:-PT15M"
                  "DESCRIPTION:Reminder" "END:VALARM"))
    nil)
(qa (qa-icalendar-duration--calendar-component-valid-p
     t "VEVENT" '("DTSTART:20250101T120000Z" "DTEND:20250101T130000Z"
                  "BEGIN:VALARM" "ACTION:DISPLAY"
                  "TRIGGER;RELATED=END:-PT15M"
                  "DESCRIPTION:Reminder" "END:VALARM"))
    t)
(qa (qa-icalendar-duration--calendar-component-valid-p
     t "VEVENT" '("DTSTART:20250101T120000Z" "DURATION:PT1H"
                  "BEGIN:VALARM" "ACTION:DISPLAY"
                  "TRIGGER;RELATED=END:-PT15M"
                  "DESCRIPTION:Reminder" "END:VALARM"))
    t)
(qa (qa-icalendar-duration--calendar-component-valid-p
     nil "VTODO" '("DTSTART:20250101T120000Z"
                   "BEGIN:VALARM" "ACTION:DISPLAY"
                   "TRIGGER;RELATED=END:-PT15M"
                   "DESCRIPTION:Reminder" "END:VALARM"))
    nil)
(qa (qa-icalendar-duration--calendar-component-valid-p
     nil "VTODO" '("DUE:20250101T130000Z"
                   "BEGIN:VALARM" "ACTION:DISPLAY"
                   "TRIGGER;RELATED=END:-PT15M"
                   "DESCRIPTION:Reminder" "END:VALARM"))
    t)
(qa (qa-icalendar-duration--calendar-component-valid-p
     nil "VTODO" '("DTSTART:20250101T120000Z" "DURATION:PT1H"
                   "BEGIN:VALARM" "ACTION:DISPLAY"
                   "TRIGGER;RELATED=END:-PT15M"
                   "DESCRIPTION:Reminder" "END:VALARM"))
    t)
(qa (qa-icalendar-duration--calendar-component-valid-p
     t "VEVENT" '("BEGIN:VALARM" "ACTION:DISPLAY"
                  "TRIGGER;VALUE=DATE-TIME:20250101T130000Z"
                  "DESCRIPTION:Reminder" "END:VALARM"))
    t)

(princ (qa--done))
