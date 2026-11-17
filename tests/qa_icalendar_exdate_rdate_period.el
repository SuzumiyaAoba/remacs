;;; qa_icalendar_exdate_rdate_period.el --- EXDATE excludes an RDATE PERIOD onset -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-utils)
(require 'icalendar-recur)

(defun qa--icalendar-rdate-period-summary (exdate-p)
  (with-temp-buffer
    (insert "BEGIN:VEVENT\n"
            "UID:rdate-period-exdate\n"
            "DTSTART:20250101T090000\n"
            "DURATION:PT1H\n"
            "RRULE:FREQ=DAILY;COUNT=2\n"
            "RDATE;VALUE=PERIOD:20250102T090000/PT2H\n"
            (if exdate-p "EXDATE:20250102T090000\n" "")
            "END:VEVENT\n")
    (goto-char (point-min))
    (let* ((event (icalendar-parse-component (point-max)))
           (lower (icalendar-read-date-time "20250101T000000"))
           (upper (icalendar-read-date-time "20250103T000000"))
           (recurrences
            (icalendar-recur-recurrences-in-window-w/end-times
             lower upper event)))
      (mapcar (lambda (interval)
                (list (icalendar-date/time-to-date (car interval))
                      (icalendar-date/time-hour (car interval))
                      (icalendar-date/time-hour (cadr interval))))
              recurrences))))

(qa
 (list (qa--icalendar-rdate-period-summary t)
       (qa--icalendar-rdate-period-summary nil))
 '((((1 1 2025) 9 10))
   (((1 1 2025) 9 10) ((1 2 2025) 9 11))))

(princ (qa--done))
