;;; qa_icalendar_recurrence_id_tzid.el --- validate RECURRENCE-ID TZID -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-recurrence-id-valid-p (line)
  (with-temp-buffer
    (insert line "\n")
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p (icalendar-parse-property (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(qa (qa--icalendar-recurrence-id-valid-p
     "RECURRENCE-ID;VALUE=DATE;TZID=Test/Zone:20250101")
    nil)
(qa (qa--icalendar-recurrence-id-valid-p
     "RECURRENCE-ID;VALUE=DATE:20250101")
    t)
(qa (qa--icalendar-recurrence-id-valid-p
     "RECURRENCE-ID;TZID=Test/Zone:20250101T090000")
    t)
(qa (qa--icalendar-recurrence-id-valid-p
     "RECURRENCE-ID;TZID=Test/Zone:20250101T090000Z")
    nil)

(princ (qa--done))
