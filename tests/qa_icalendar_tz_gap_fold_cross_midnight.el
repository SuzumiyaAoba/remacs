;;; qa_icalendar_tz_gap_fold_cross_midnight.el --- detect DST gaps and folds across midnight -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-utils)
(require 'icalendar-recur)

(defun qa-icalendar--transition-result
    (component-name component-type onset-string candidate-string predicate offset-from offset-to)
  (with-temp-buffer
    (insert "BEGIN:VTIMEZONE\n"
            "TZID:Test/Edge\n"
            "BEGIN:" component-name "\n"
            "DTSTART:" onset-string "\n"
            "TZOFFSETFROM:" offset-from "\n"
            "TZOFFSETTO:" offset-to "\n"
            "END:" component-name "\n"
            "END:VTIMEZONE\n")
    (goto-char (point-min))
    (let* ((vtimezone (icalendar-parse-component (point-max)))
           (observance
            (car (icalendar-ast-node-children-of component-type vtimezone)))
           (onset (icalendar-read-date-time onset-string))
           (candidate (icalendar-read-date-time candidate-string)))
      (funcall predicate candidate onset observance))))

(qa
 (list
  (qa-icalendar--transition-result
   "DAYLIGHT" 'icalendar-daylight "20241231T233000" "20250101T001500"
   #'icalendar-recur-nonexistent-date-time-p "-0500" "-0400")
  (qa-icalendar--transition-result
   "DAYLIGHT" 'icalendar-daylight "20241231T233000" "20250101T004500"
   #'icalendar-recur-nonexistent-date-time-p "-0500" "-0400")
  (qa-icalendar--transition-result
   "STANDARD" 'icalendar-standard "20250101T003000" "20241231T234500"
   #'icalendar-recur-date-time-occurs-twice-p "-0400" "-0500")
  (qa-icalendar--transition-result
   "STANDARD" 'icalendar-standard "20250101T003000" "20241231T231500"
   #'icalendar-recur-date-time-occurs-twice-p "-0400" "-0500"))
 '(t nil t nil))

(princ (qa--done))
