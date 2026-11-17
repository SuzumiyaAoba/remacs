;;; qa_icalendar_unknown_param_values.el --- preserve unknown parameter value lists -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-unknown-param-values (line)
  (with-temp-buffer
    (insert line "\n")
    (goto-char (point-min))
    (let ((property (icalendar-parse-property (point-max))))
      (mapcar #'icalendar-ast-node-value
              (icalendar-ast-node-children-of 'icalendar-otherparam property)))))

(qa (qa--icalendar-unknown-param-values
     "SUMMARY;X-FOO=one,two:Meeting")
    '(("one" "two")))
(qa (qa--icalendar-unknown-param-values
     "SUMMARY;X-FOO=\"one,two\",three:Meeting")
    '(("one,two" "three")))
(qa (qa--icalendar-unknown-param-values
     "SUMMARY;X-FOO=:Meeting")
    '(("")))
(qa (qa--icalendar-unknown-param-values
     "SUMMARY;X-EMPTY=;X-FOO=one,two:Meeting")
    '(("") ("one" "two")))

(princ (qa--done))
