;;; -*- lexical-binding: t; -*-
;; RDATE and EXDATE use their own TZID when diary recurrences are evaluated (#1986).

(require 'diary-icalendar)
(require 'icalendar-parser)

(defvar date)
(defvar entry)

(defun qa-diary-icalendar-rdate-tzid-result ()
  (with-temp-buffer
    (insert
     (concat
      "BEGIN:VCALENDAR\nVERSION:2.0\n"
      "BEGIN:VTIMEZONE\nTZID:America/New_York\n"
      "BEGIN:STANDARD\nDTSTART:20241103T020000\n"
      "TZOFFSETFROM:-0400\nTZOFFSETTO:-0500\nTZNAME:EST\nEND:STANDARD\n"
      "BEGIN:DAYLIGHT\nDTSTART:20240310T020000\n"
      "TZOFFSETFROM:-0500\nTZOFFSETTO:-0400\nTZNAME:EDT\nEND:DAYLIGHT\n"
      "END:VTIMEZONE\n"
      "BEGIN:VTIMEZONE\nTZID:Asia/Tokyo\n"
      "BEGIN:STANDARD\nDTSTART:19700101T000000\n"
      "TZOFFSETFROM:+0900\nTZOFFSETTO:+0900\nTZNAME:JST\nEND:STANDARD\n"
      "END:VTIMEZONE\n"
      "BEGIN:VEVENT\nUID:multiple-timezones\n"
      "DTSTART;TZID=America/New_York:20250106T090000\n"
      "RRULE:FREQ=WEEKLY;BYDAY=MO\n"
      "RDATE;TZID=Asia/Tokyo:20250107T090000,20250107T100000\n"
      "EXDATE;TZID=Asia/Tokyo:20250107T090000\n"
      "SUMMARY:Meeting\nEND:VEVENT\nEND:VCALENDAR\n"))
    (let* ((parsed (icalendar-parse-and-index))
           (index (cadr parsed))
           (component (car (icalendar-ast-node-children-of
                            'icalendar-vevent (car parsed))))
           (diary-entry (diary-icalendar-format-rrule-sexp component index))
           (sexp (car (read-from-string
                       (substring diary-entry
                                  (length diary-sexp-entry-symbol))))))
      (calendar-dlet ((date '(1 6 2025)) (entry "Meeting"))
        (eval sexp nil)))))

(qa
 (unwind-protect
     (progn
       (set-time-zone-rule "EST5EDT,M3.2.0,M11.1.0")
       (let ((result (qa-diary-icalendar-rdate-tzid-result)))
         (list
          (not (null (string-match-p "09:00 Meeting" result)))
          (not (null (string-match-p "20:00 Meeting" result)))
          (not (string-match-p "19:00 Meeting" result)))))
   (set-time-zone-rule 'wall))
 '(t t t))

(princ (qa--done))
