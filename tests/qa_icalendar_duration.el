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

(defun qa-icalendar-duration--parse-calendar (content)
  (with-temp-buffer
    (insert "BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:-//qa//remacs//EN\n"
            content "\nEND:VCALENDAR")
    (goto-char (point-min))
    (icalendar-parse-calendar (point-max))))

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

(defun qa-icalendar-duration--utc-offset-valid-p (text)
  (condition-case nil
      (progn (icalendar-parse-from-string 'icalendar-utc-offset text) t)
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

;; VTIMEZONE RDATE onsets must be local even without RRULE (#2533).
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/RDATE\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0000\nTZOFFSETTO:+0000\n"
          "RDATE:20250101T000000\nEND:STANDARD\nEND:VTIMEZONE"))
    t)
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/RDATE\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0000\nTZOFFSETTO:+0000\n"
          "RDATE:20250101T000000Z\nEND:STANDARD\nEND:VTIMEZONE"))
    nil)

;; TZID applies to every DATE-TIME in list-valued RDATE/EXDATE; it is invalid
;; on DATE or UTC values in those lists (#2534/#2535).
(qa (let* ((calendar
            (qa-icalendar-duration--parse-calendar
             (concat
              "BEGIN:VTIMEZONE\nTZID:Test/PlusOne\n"
              "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
              "TZOFFSETFROM:+0100\nTZOFFSETTO:+0100\nEND:STANDARD\n"
              "END:VTIMEZONE\n"
              "BEGIN:VEVENT\nUID:tzid-lists\n"
              "DTSTAMP:20250101T000000Z\n"
              "DTSTART;TZID=Test/PlusOne:20250101T090000\n"
              "RDATE;TZID=Test/PlusOne:20250102T090000,20250103T090000\n"
              "EXDATE;TZID=Test/PlusOne:20250104T090000,20250105T090000\n"
              "END:VEVENT")))
           (event (car (icalendar-ast-node-children-of 'icalendar-vevent calendar)))
           (dtstart (icalendar-ast-node-value
                     (icalendar-ast-node-value
                      (icalendar-ast-node-first-child-of 'icalendar-dtstart event))))
           (rdate (icalendar-ast-node-value
                   (icalendar-ast-node-first-child-of 'icalendar-rdate event)))
           (exdate (icalendar-ast-node-value
                    (icalendar-ast-node-first-child-of 'icalendar-exdate event))))
      (list (decoded-time-zone dtstart)
            (mapcar (lambda (node)
                      (decoded-time-zone (icalendar-ast-node-value node)))
                    rdate)
            (mapcar (lambda (node)
                      (decoded-time-zone (icalendar-ast-node-value node)))
                    exdate)))
    '(3600 (3600 3600) (3600 3600)))
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/PlusOne\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0100\nTZOFFSETTO:+0100\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VEVENT\nUID:bad-rdate-tzid\n"
          "DTSTAMP:20250101T000000Z\n"
          "DTSTART;TZID=Test/PlusOne:20250101T090000\n"
          "RDATE;TZID=Test/PlusOne:20250102T090000Z,20250103T090000Z\n"
          "END:VEVENT"))
    nil)
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/PlusOne\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0100\nTZOFFSETTO:+0100\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VEVENT\nUID:bad-exdate-tzid\n"
          "DTSTAMP:20250101T000000Z\n"
          "DTSTART;TZID=Test/PlusOne:20250101T090000\n"
          "EXDATE;TZID=Test/PlusOne;VALUE=DATE:20250104,20250105\n"
          "END:VEVENT"))
    nil)
(qa (qa-icalendar-duration--calendar-content-valid-p
     nil (concat
          "BEGIN:VTIMEZONE\nTZID:Test/PlusOne\n"
          "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
          "TZOFFSETFROM:+0100\nTZOFFSETTO:+0100\nEND:STANDARD\n"
          "END:VTIMEZONE\n"
          "BEGIN:VEVENT\nUID:bad-period-tzid\n"
          "DTSTAMP:20250101T000000Z\n"
          "DTSTART;TZID=Test/PlusOne:20250101T090000\n"
          "RDATE;TZID=Test/PlusOne;VALUE=PERIOD:"
          "20250102T090000Z/20250102T100000Z\n"
          "END:VEVENT"))
    nil)

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

;; SEQUENCE is a non-negative revision counter (#2536).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("SEQUENCE:-1")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("SEQUENCE:0")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("SEQUENCE:3")))
    t)

;; GEO coordinates stay within latitude/longitude limits (#2528).
(qa (qa-icalendar-duration--geo-valid-p "91.0;0.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "-91.0;0.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "0.0;181.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "0.0;-181.0") nil)
(qa (qa-icalendar-duration--geo-valid-p "90.0;180.0") t)
(qa (qa-icalendar-duration--geo-valid-p "-90.0;-180.0") t)

;; UTC-OFFSET fields and negative zero are constrained by RFC 5545 (#2531).
(qa (qa-icalendar-duration--utc-offset-valid-p "+2500") nil)
(qa (qa-icalendar-duration--utc-offset-valid-p "+0060") nil)
(qa (qa-icalendar-duration--utc-offset-valid-p "+000060") nil)
(qa (qa-icalendar-duration--utc-offset-valid-p "-0000") nil)
(qa (qa-icalendar-duration--utc-offset-valid-p "-000000") nil)
(qa (qa-icalendar-duration--utc-offset-valid-p "+0000") t)
(qa (qa-icalendar-duration--utc-offset-valid-p "+2359") t)
(qa (qa-icalendar-duration--utc-offset-valid-p "+235959") t)
(qa (qa-icalendar-duration--utc-offset-valid-p "-000001") t)

;; LAST-MODIFIED values are UTC timestamps (#2537).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT"
      '("LAST-MODIFIED:20250101T010000")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT"
      '("LAST-MODIFIED:20250101T010000Z")))
    t)

;; LANGUAGE subtags must be nonempty and separated by single hyphens (#2538).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("SUMMARY;LANGUAGE=en--US:Meeting")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("SUMMARY;LANGUAGE=-en:Meeting")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("SUMMARY;LANGUAGE=en-:Meeting")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("SUMMARY;LANGUAGE=en-US:Meeting")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("SUMMARY;LANGUAGE=zh-Hant-TW:Meeting")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("SUMMARY;LANGUAGE=de-CH-1901:Meeting")))
    t)

;; STATUS and ATTENDEE PARTSTAT use their component-specific value sets (#2529/#2530).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("STATUS:COMPLETED")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VEVENT" '("STATUS:CONFIRMED")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VTODO" '("STATUS:TENTATIVE")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VTODO" '("STATUS:COMPLETED")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VJOURNAL" '("STATUS:NEEDS-ACTION")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component "VJOURNAL" '("STATUS:DRAFT")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("ATTENDEE;PARTSTAT=COMPLETED:mailto:person@example.org")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("ATTENDEE;PARTSTAT=TENTATIVE:mailto:person@example.org")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("ATTENDEE;PARTSTAT=IN-PROCESS:mailto:person@example.org")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VTODO" '("ATTENDEE;PARTSTAT=COMPLETED:mailto:person@example.org")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VJOURNAL" '("ATTENDEE;PARTSTAT=TENTATIVE:mailto:person@example.org")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VJOURNAL" '("ATTENDEE;PARTSTAT=ACCEPTED:mailto:person@example.org")))
    t)

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

;; REPEAT counts additional alarm firings and cannot be negative (#2539).
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250101T120000Z" "BEGIN:VALARM"
                 "ACTION:DISPLAY" "TRIGGER:-PT5M" "REPEAT:-1"
                 "DURATION:PT1M" "DESCRIPTION:Reminder" "END:VALARM")))
    nil)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250101T120000Z" "BEGIN:VALARM"
                 "ACTION:DISPLAY" "TRIGGER:-PT5M" "REPEAT:0"
                 "DURATION:PT1M" "DESCRIPTION:Reminder" "END:VALARM")))
    t)
(qa (qa-icalendar-duration--component-valid-p
     (qa-icalendar-duration--component
      "VEVENT" '("DTSTART:20250101T120000Z" "BEGIN:VALARM"
                 "ACTION:DISPLAY" "TRIGGER:-PT5M" "REPEAT:2"
                 "DURATION:PT1M" "DESCRIPTION:Reminder" "END:VALARM")))
    t)

(princ (qa--done))
