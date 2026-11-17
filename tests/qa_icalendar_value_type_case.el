;;; qa_icalendar_value_type_case.el --- read VALUE names case-insensitively -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-property-value-type (line)
  (with-temp-buffer
    (insert line "\n")
    (goto-char (point-min))
    (let ((property (icalendar-parse-property (point-max))))
      (icalendar-ast-node-valid-p property t)
      (icalendar-ast-node-type (icalendar-ast-node-value property)))))

(qa (icalendar-read-value-type "date") 'icalendar-date)
(qa (icalendar-read-value-type "date-time") 'icalendar-date-time)
(qa (icalendar-read-value-type "x-custom-value") "x-custom-value")
(qa (qa--icalendar-property-value-type "DTSTART;VALUE=date:20250101")
    'icalendar-date)

(princ (qa--done))
