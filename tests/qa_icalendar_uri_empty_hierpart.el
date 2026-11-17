;;; qa_icalendar_uri_empty_hierpart.el --- accept empty URI paths -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(qa (not (null (icalendar-uri-valid-p "foo:"))) t)
(qa (not (null (icalendar-uri-valid-p "foo:#fragment"))) t)
(qa (icalendar-ast-node-value
     (icalendar-parse-from-string 'icalendar-uri "foo:"))
    "foo:")

(princ (qa--done))
