;;; qa_icalendar_email_attendee_mailto.el --- validate EMAIL alarm recipients -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-email-attendee-valid-p (address)
  (with-temp-buffer
    (insert "BEGIN:VEVENT\nUID:email-alarm-recipient\n"
            "DTSTAMP:20250101T000000Z\n"
            "DTSTART:20250101T120000Z\n"
            "BEGIN:VALARM\nACTION:EMAIL\nTRIGGER:-PT5M\n"
            "SUMMARY:Reminder\nDESCRIPTION:Body\n"
            "ATTENDEE:" address "\n"
            "END:VALARM\nEND:VEVENT\n")
    (goto-char (point-min))
    (condition-case nil
        (progn
          (icalendar-ast-node-valid-p
           (icalendar-parse-component (point-max)) t)
          t)
      (icalendar-validation-error nil))))

(qa (qa--icalendar-email-attendee-valid-p "ftp://example.com/person") nil)
(qa (qa--icalendar-email-attendee-valid-p "mailto:person@example.com") t)
(qa (qa--icalendar-email-attendee-valid-p "MAILTO:person@example.com") t)

(princ (qa--done))
