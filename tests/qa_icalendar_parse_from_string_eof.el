;;; qa_icalendar_parse_from_string_eof.el --- parse properties without final newline -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-summary-value (string)
  (icalendar-text-to-string
   (icalendar-ast-node-value
    (icalendar-parse-from-string 'icalendar-summary string))))

(qa (qa--icalendar-summary-value "SUMMARY:hello") "hello")
(qa (qa--icalendar-summary-value "SUMMARY:hello\n") "hello")

(princ (qa--done))
