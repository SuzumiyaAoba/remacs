;;; qa_icalendar_historic_timezone.el --- use historical VTIMEZONE rules -*- lexical-binding: t; -*-

(require 'icalendar)
(require 'icalendar-parser)
(require 'icalendar-recur)

(defun qa--icalendar-historic-event-hour-utc ()
  (with-temp-buffer
    (insert "BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:-//qa//EN\n"
            "BEGIN:VTIMEZONE\nTZID:Test/Eastern\n"
            "BEGIN:STANDARD\nDTSTART:19701025T020000\n"
            "RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\n"
            "TZOFFSETFROM:-0400\nTZOFFSETTO:-0500\nEND:STANDARD\n"
            "BEGIN:DAYLIGHT\nDTSTART:19870405T020000\n"
            "RRULE:FREQ=YEARLY;BYMONTH=4;BYDAY=1SU\n"
            "TZOFFSETFROM:-0500\nTZOFFSETTO:-0400\nEND:DAYLIGHT\n"
            "BEGIN:STANDARD\nDTSTART:20071104T020000\n"
            "RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\n"
            "TZOFFSETFROM:-0400\nTZOFFSETTO:-0500\nEND:STANDARD\n"
            "BEGIN:DAYLIGHT\nDTSTART:20070311T020000\n"
            "RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\n"
            "TZOFFSETFROM:-0500\nTZOFFSETTO:-0400\nEND:DAYLIGHT\n"
            "END:VTIMEZONE\nEND:VCALENDAR\n")
    (goto-char (point-min))
    (let* ((ical (icalendar--read-element nil nil))
           (zone-map (icalendar--convert-all-timezones ical))
           (zone (cdr (assoc "Test/Eastern" zone-map)))
           (decoded
            (icalendar--decode-isodatetime
             "20050320T090000" nil zone t)))
      (decoded-time-hour decoded))))

(qa (qa--icalendar-historic-event-hour-utc) 14)

(princ (qa--done))
