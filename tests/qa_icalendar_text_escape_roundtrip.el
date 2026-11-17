;;; qa_icalendar_text_escape_roundtrip.el --- decode escaped TEXT left to right -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(qa
 (let* ((literal-backslash-n (concat (string 92) "n"))
        (escaped (icalendar-escape-text-string literal-backslash-n))
        (decoded (icalendar-unescape-text-string escaped)))
   (list (equal decoded literal-backslash-n)
         (equal (icalendar-unescape-text-string
                 (concat (string 92) "n"))
                (string 10))
         (equal (icalendar-unescape-text-string
                 (concat (string 92) (string 92) "n"))
                literal-backslash-n)
         (equal (icalendar-unescape-text-string
                 (concat (string 92) "x"))
                (concat (string 92) "x"))
         (equal (icalendar-unescape-text-string
                 (concat (string 92) ","))
                ",")))
 '(t t t t t))

(princ (qa--done))
