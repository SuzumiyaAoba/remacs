;;; qa_icalendar_member_quoted_comma.el --- preserve quoted list commas -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-member-values (string)
  (let* ((node (car (icalendar-parse-from-string 'icalendar-memberparam string)))
         (values (icalendar-ast-node-value node)))
    (mapcar #'icalendar-ast-node-value values)))

(qa (qa--icalendar-member-values
     "MEMBER=\"https://example.com/a,b\"")
    '("https://example.com/a,b"))
(qa (qa--icalendar-member-values
     "MEMBER=\"https://example.com/a,b\",\"https://example.com/c,d\"")
    '("https://example.com/a,b" "https://example.com/c,d"))

(princ (qa--done))
