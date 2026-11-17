;;; qa_icalendar_date_time_case.el --- validate DATE-TIME separator case -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-parsed-value (type string strict)
  (let ((icalendar-parse-strictly strict))
    (icalendar-ast-node-value (icalendar-parse-from-string type string))))

(defun qa--icalendar-value-parse-error-p (type string strict)
  (let ((icalendar-parse-strictly strict))
    (condition-case nil
        (progn (icalendar-parse-from-string type string) nil)
      (icalendar-parse-error t))))

(qa (icalendar-date-time-is-utc-p
     (qa--icalendar-parsed-value 'icalendar-date-time "20250101T090000z" nil))
    t)
(qa (icalendar-print-date-time
     (qa--icalendar-parsed-value 'icalendar-date-time "20250101t090000z" nil))
    "20250101T090000Z")
(qa (icalendar-date-time-is-utc-p
     (qa--icalendar-parsed-value 'icalendar-date-time "20250101T090000Z" t))
    t)
(qa (qa--icalendar-value-parse-error-p
     'icalendar-date-time "20250101t090000Z" t)
    t)
(qa (qa--icalendar-value-parse-error-p
     'icalendar-date-time "20250101T090000z" t)
    t)
(qa (eq t (decoded-time-zone
           (qa--icalendar-parsed-value 'icalendar-time "090000z" nil)))
    t)
(qa (qa--icalendar-value-parse-error-p 'icalendar-time "090000z" t) t)

(princ (qa--done))
