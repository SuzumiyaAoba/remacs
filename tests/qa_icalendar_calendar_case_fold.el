;;; qa_icalendar_calendar_case_fold.el --- parse delimiters independent of search case -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-recur)

(qa (let ((case-fold-search nil))
      (with-temp-buffer
        (insert "begin:vcalendar\nVERSION:2.0\nPRODID:-//example//EN\n"
                "begin:vevent\nUID:lower-begin\n"
                "DTSTAMP:20250101T000000Z\nDTSTART:20250101T090000Z\n"
                "end:vevent\nend:vcalendar\n")
        (goto-char (point-min))
        (let ((calendar (icalendar-parse-calendar (point-max))))
          (and (eq 'icalendar-vcalendar (icalendar-ast-node-type calendar))
               (icalendar-ast-node-valid-p calendar t)
               t))))
    t)
(qa (let ((case-fold-search nil))
      (with-temp-buffer
        (insert "begin:VCALENDAR\n")
        (icalendar-contains-vcalendar-p)))
    1)

(princ (qa--done))
