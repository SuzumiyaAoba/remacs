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

;; GNU 31.1 currently emits invalid BYYEARDAY=0 for this case.  The offset
;; must either be represented correctly or rejected before emitting a node.
(defun qa--diary-offset-year-day (year-day ndays)
  (let* ((nodes
          (diary-icalendar-offset-sexp-to-nodes
           `(diary-offset
             (diary-rrule :start '(1 1 2025)
                          :rule '((FREQ YEARLY) (BYYEARDAY (,year-day))))
             ,ndays)))
         (rrule-node (cadr nodes))
         (value (icalendar-ast-node-value
                 (icalendar-ast-node-value rrule-node))))
    (list (caar (alist-get 'BYYEARDAY value))
          (icalendar-rrule-value-p value))))

(qa (qa--diary-offset-year-day 1 -1) '(-1 t))
(qa (qa--diary-offset-year-day 1 -2) '(-2 t))
(qa (qa--diary-offset-year-day -1 1) '(1 t))
(qa (condition-case nil
        (progn (qa--diary-offset-year-day 365 1) nil)
      (icalendar-diary-export-error t))
    t)
(qa (condition-case nil
        (progn (qa--diary-offset-year-day 366 -1) nil)
      (icalendar-diary-export-error t))
    t)

(princ (qa--done))
