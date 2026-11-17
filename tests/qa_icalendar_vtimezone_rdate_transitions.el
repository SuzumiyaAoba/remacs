;;; qa_icalendar_vtimezone_rdate_transitions.el --- use explicit VTIMEZONE RDATE transitions -*- lexical-binding: t; -*-

(require 'icalendar)

(qa
 (let* ((daylight
         (list 'DAYLIGHT nil
               (list (list 'DTSTART nil "20240310T020000")
                     (list 'TZOFFSETFROM nil "-0500")
                     (list 'TZOFFSETTO nil "-0400")
                     (list 'RDATE nil "20250309T020000,20260308T020000"))
               nil))
        (standard
         (list 'STANDARD nil
               (list (list 'DTSTART nil "20241103T020000")
                     (list 'TZOFFSETFROM nil "-0400")
                     (list 'TZOFFSETTO nil "-0500")
                     (list 'RDATE nil "20251102T020000,20261101T020000"))
               nil))
        (vtimezone
         (list 'VTIMEZONE nil
               (list (list 'TZID nil "Test/RDATE"))
               (list daylight standard)))
        (zone-map (icalendar--convert-all-timezones
                   (list (list 'VCALENDAR nil nil (list vtimezone)))))
        (zone (icalendar--find-time-zone '(TZID "Test/RDATE") zone-map)))
   (list (icalendar--convert-tz-offset (caddr daylight) t)
         (icalendar--rdate-zone-offset zone "20250309T013000")
         (icalendar--rdate-zone-offset zone "20250309T023000")
         (icalendar--rdate-zone-offset zone "20260308T023000")
         (icalendar--rdate-zone-offset zone "20251102T013000")
         (icalendar--rdate-zone-offset zone "20251102T023000")
         (nth 2 (icalendar--decode-isodatetime
                 "20250309T030000" nil zone t))))
 '(nil -18000 -14400 -14400 -14400 -18000 7))

(princ (qa--done))
