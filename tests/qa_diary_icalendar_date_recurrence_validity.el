;;; qa_diary_icalendar_date_recurrence_validity.el --- filter invalid diary dates -*- lexical-binding: t; -*-

(require 'diary-icalendar)
(require 'icalendar-parser)

(defun qa--diary-date-recurrence-summary (sexp start-year)
  (let* ((calendar-date-style 'american)
         (diary-icalendar-recurring-start-year start-year)
         (nodes (diary-icalendar-date-sexp-to-recurrence sexp))
         (start (car nodes))
         (recurrence (cadr nodes))
         (rdates
          (when (and recurrence
                     (eq 'icalendar-rdate (icalendar-ast-node-type recurrence)))
            (mapcar #'icalendar-ast-node-value
                    (icalendar-ast-node-value recurrence)))))
    (list (icalendar-ast-node-value (icalendar-ast-node-value start)) rdates)))

(qa (qa--diary-date-recurrence-summary '(diary-date t 31 (2025)) 2025)
    '((1 31 2025)
      ((3 31 2025) (5 31 2025) (7 31 2025) (8 31 2025)
       (10 31 2025) (12 31 2025))))
(qa (qa--diary-date-recurrence-summary '(diary-date (2 3) 31 t) 2025)
    '((3 31 2025) nil))
(qa (qa--diary-date-recurrence-summary '(diary-date 2 29 t) 2025)
    '((2 29 2028) nil))
(qa (condition-case nil
        (progn
          (qa--diary-date-recurrence-summary '(diary-date 2 30 (2025)) 2025)
          nil)
      (icalendar-diary-export-error t))
    t)

(princ (qa--done))
