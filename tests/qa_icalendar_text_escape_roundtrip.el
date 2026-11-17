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

(defun qa--icalendar-categories (text)
  (with-temp-buffer
    (insert text "\n")
    (goto-char (point-min))
    (mapcar #'icalendar-text-to-string
            (icalendar-ast-node-value (icalendar-parse-property (point-max))))))

(qa (qa--icalendar-categories
     (concat "CATEGORIES:foo" (string 92) ",bar"))
    '("foo,bar"))
(qa (qa--icalendar-categories
     (concat "CATEGORIES:foo" (string 92) ",bar,baz"))
    '("foo,bar" "baz"))

(princ (qa--done))
