;;; qa_icalendar_rrule_until.el --- validate RRULE UNTIL -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-rrule-valid-p (text)
  (with-temp-buffer
    (insert text)
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:date-until-type\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART;VALUE=DATE:20250101\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000Z\n"
             "END:VEVENT"))
    nil)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:date-until\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART;VALUE=DATE:20250101\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131\nEND:VEVENT"))
    t)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:floating-until-utc\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART:20250101T090000\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000Z\n"
             "END:VEVENT"))
    nil)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:floating-until\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART:20250101T090000\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000\nEND:VEVENT"))
    t)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:utc-until-floating\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART:20250101T090000Z\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000\nEND:VEVENT"))
    nil)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:utc-until\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART:20250101T090000Z\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000Z\nEND:VEVENT"))
    t)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:tzid-until-floating\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART;TZID=Test/Zone:20250101T090000\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000\nEND:VEVENT"))
    nil)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:tzid-until-utc\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART;TZID=Test/Zone:20250101T090000\n"
             "RRULE:FREQ=DAILY;UNTIL=20250131T000000Z\nEND:VEVENT"))
    t)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VEVENT\nUID:date-time-rule\n"
             "DTSTAMP:20250101T000000Z\n"
             "DTSTART;VALUE=DATE:20250101\n"
             "RRULE:FREQ=HOURLY\nEND:VEVENT"))
    nil)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VTIMEZONE\nTZID:Test/Zone\nBEGIN:STANDARD\n"
             "DTSTART:20251102T020000\nTZOFFSETFROM:-0400\n"
             "TZOFFSETTO:-0500\n"
             "RRULE:FREQ=YEARLY;UNTIL=20261231T000000Z\n"
             "END:STANDARD\nEND:VTIMEZONE"))
    t)
(qa (qa--icalendar-rrule-valid-p
     (concat "BEGIN:VTIMEZONE\nTZID:Test/Zone\nBEGIN:STANDARD\n"
             "DTSTART:20251102T020000\nTZOFFSETFROM:-0400\n"
             "TZOFFSETTO:-0500\n"
             "RRULE:FREQ=YEARLY;UNTIL=20261231T000000\n"
             "END:STANDARD\nEND:VTIMEZONE"))
    nil)

(princ (qa--done))
