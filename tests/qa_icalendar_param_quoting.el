;;; qa_icalendar_param_quoting.el --- quote parameter values safely -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-param-with-value (value)
  (let ((param (car (icalendar-parse-from-string 'icalendar-cnparam "CN=foo"))))
    (icalendar-ast-node-set-value param value)
    param))

(qa (icalendar-maybe-quote-param-value "safe") "safe")
(qa (icalendar-maybe-quote-param-value "a,b") "\"a,b\"")
(qa (icalendar-maybe-quote-param-value ",abc") "\",abc\"")
(qa (icalendar-print-param-node (qa--icalendar-param-with-value ",abc"))
    ";CN=\",abc\"")
(qa (condition-case nil
        (progn
          (icalendar-print-param-node
           (qa--icalendar-param-with-value "foo\"bar"))
          nil)
      (icalendar-print-error t))
    t)

(princ (qa--done))
