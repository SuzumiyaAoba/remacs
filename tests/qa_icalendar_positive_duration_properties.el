;;; qa_icalendar_positive_duration_properties.el --- reject negative component DURATION values -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-utils)

(defun qa--icalendar-component-errors-p (text)
  (with-temp-buffer
    (insert text)
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          nil)
      (error t))))

(qa
 (list
  (qa--icalendar-component-errors-p
   (concat "BEGIN:VEVENT\n"
           "UID:negative-duration\n"
           "DTSTAMP:20250101T000000Z\n"
           "DTSTART:20250101T120000Z\n"
           "DURATION:-PT1H\n"
           "END:VEVENT\n"))
  (qa--icalendar-component-errors-p
   (concat "BEGIN:VTODO\n"
           "UID:negative-todo-duration\n"
           "DTSTAMP:20250101T000000Z\n"
           "DTSTART:20250101T120000Z\n"
           "DURATION:-PT1H\n"
           "END:VTODO\n"))
  (qa--icalendar-component-errors-p
   (concat "BEGIN:VEVENT\n"
           "UID:negative-alarm-duration\n"
           "DTSTAMP:20250101T000000Z\n"
           "DTSTART:20250101T120000Z\n"
           "BEGIN:VALARM\n"
           "ACTION:DISPLAY\n"
           "TRIGGER:-PT5M\n"
           "REPEAT:2\n"
           "DURATION:-PT1M\n"
           "DESCRIPTION:Reminder\n"
           "END:VALARM\n"
           "END:VEVENT\n"))
  (qa--icalendar-component-errors-p
   (concat "BEGIN:VEVENT\n"
           "UID:zero-duration\n"
           "DTSTAMP:20250101T000000Z\n"
           "DTSTART:20250101T120000Z\n"
           "DURATION:PT0S\n"
           "END:VEVENT\n"))
  (not (qa--icalendar-component-errors-p
        (concat "BEGIN:VEVENT\n"
                "UID:positive-duration\n"
                "DTSTAMP:20250101T000000Z\n"
                "DTSTART:20250101T120000Z\n"
                "DURATION:PT1H\n"
                "END:VEVENT\n")))
  (not (qa--icalendar-component-errors-p
        (concat "BEGIN:VEVENT\n"
                "UID:negative-trigger\n"
                "DTSTAMP:20250101T000000Z\n"
                "DTSTART:20250101T120000Z\n"
                "BEGIN:VALARM\n"
                "ACTION:DISPLAY\n"
                "TRIGGER:-PT5M\n"
                "REPEAT:2\n"
                "DURATION:PT1M\n"
                "DESCRIPTION:Reminder\n"
                "END:VALARM\n"
                "END:VEVENT\n"))))
 '(t t t t t t))

(princ (qa--done))
