;;; qa_icalendar_rrule_weekday_case.el --- parse weekday tokens case-insensitively -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-rrule-weekdays (rule)
  (with-temp-buffer
    (insert "BEGIN:VEVENT\nUID:lower-weekday\n"
            "DTSTAMP:20250101T000000Z\n"
            "DTSTART:20250101T090000Z\n"
            "RRULE:" rule "\n"
            "END:VEVENT\n")
    (goto-char (point-min))
    (let* ((event (icalendar-parse-component (point-max)))
           (rrule (icalendar-ast-node-first-child-of 'icalendar-rrule event))
           (value (icalendar-ast-node-value (icalendar-ast-node-value rrule))))
      (icalendar-ast-node-valid-p event t)
      (list (car (alist-get 'FREQ value))
            (car (alist-get 'BYDAY value))
            (car (alist-get 'WKST value))))))

(qa (let ((case-fold-search nil))
      (qa--icalendar-rrule-weekdays "FREQ=weekly;byday=mo;wkst=su"))
    '(WEEKLY (1) 0))
(qa (let ((case-fold-search nil))
      (icalendar-read-weekdaynum "mo"))
    1)

(princ (qa--done))
