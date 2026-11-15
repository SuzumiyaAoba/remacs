;;; -*- lexical-binding: t; -*-
;;; Validate the yearly HTML calendar's index column count (Issue #2297).

(require 'cal-html)

(defun qa-cal-html-invalid-index-cols (cols)
  (let ((cal-html-year-index-cols cols))
    (with-temp-buffer
      (let ((error-data
             (condition-case data
                 (cal-html-insert-year-minicals 2024 cal-html-year-index-cols)
               (error data))))
        (list (car error-data) (cadr error-data) (buffer-string))))))

(qa (mapcar #'qa-cal-html-invalid-index-cols '(0 -1))
    '((args-out-of-range 0 "") (args-out-of-range -1 "")))

(qa (with-temp-buffer
      (cal-html-insert-year-minicals 2024 3)
      (> (buffer-size) 0))
    t)

(princ (qa--done))
