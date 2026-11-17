;;; qa_icalendar_case_fold_boolean.el --- parse BOOLEAN case-insensitively -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-boolean-value (string)
  (let ((case-fold-search nil))
    (icalendar-ast-node-value
     (icalendar-parse-from-string 'icalendar-boolean string))))

(defun qa--icalendar-rsvp-value (string)
  (let* ((case-fold-search nil)
         (param (car (icalendar-parse-from-string
                      'icalendar-rsvpparam (concat "RSVP=" string)))))
    (icalendar-ast-node-value (icalendar-ast-node-value param))))

(qa (qa--icalendar-boolean-value "true") t)
(qa (qa--icalendar-boolean-value "TrUe") t)
(qa (qa--icalendar-boolean-value "false") nil)
(qa (qa--icalendar-rsvp-value "true") t)
(qa (qa--icalendar-rsvp-value "FaLsE") nil)

(princ (qa--done))
