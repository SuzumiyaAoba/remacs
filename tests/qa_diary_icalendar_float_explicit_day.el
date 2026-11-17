;;; qa_diary_icalendar_float_explicit_day.el --- preserve explicit diary-float days -*- lexical-binding: t; -*-

(require 'diary-icalendar)
(require 'icalendar-parser)

(defun qa--diary-float-occurrences-in-window (sexp start-year lower upper)
  (let* ((calendar-date-style 'american)
         (diary-icalendar-recurring-start-year start-year)
         (nodes (diary-icalendar-float-sexp-to-recurrence sexp))
         (start-node (car nodes))
         (rdate-node (cadr nodes))
         (start (icalendar-ast-node-value
                 (icalendar-ast-node-value start-node)))
         (rdates (when rdate-node
                   (mapcar #'icalendar-ast-node-value
                           (icalendar-ast-node-value rdate-node))))
         (dates (seq-filter
                 (lambda (date)
                   (and (icalendar-date/time<= lower date)
                        (icalendar-date/time< date upper)))
                 (cons start rdates))))
    (sort dates #'icalendar-date/time<)))

(qa (qa--diary-float-occurrences-in-window
     '(diary-float 9 1 -1 15) 2024 '(9 1 2024) '(10 1 2025))
    '((9 9 2024) (9 15 2025)))
(qa (qa--diary-float-occurrences-in-window
     '(diary-float 1 1 1 31) 2025 '(1 1 2025) '(1 1 2027))
    '((2 3 2025) (2 2 2026)))
(qa (qa--diary-float-occurrences-in-window
     '(diary-float 1 1 -2 7) 2025 '(12 1 2024) '(1 1 2026))
    '((12 30 2024) (12 29 2025)))
(qa (qa--diary-float-occurrences-in-window
     '(diary-float 1 1 1 24) 2024 '(1 1 2024) '(2 1 2025))
    '((1 29 2024) (1 27 2025)))

(princ (qa--done))
