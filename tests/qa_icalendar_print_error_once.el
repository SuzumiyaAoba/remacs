;;; qa_icalendar_print_error_once.el --- print one diagnostic per error -*- lexical-binding: t; -*-

(require 'icalendar-parser)

(defun qa--icalendar-print-error-line-count ()
  (let* ((icalendar-error-buffer-name "*icalendar-print-error-qa*")
         (icalendar-debug-level 1)
         (buffer (get-buffer-create icalendar-error-buffer-name)))
    (unwind-protect
        (progn
          (with-current-buffer buffer (erase-buffer))
          (icalendar-handle-print-error
           '(icalendar-print-error :message "test printing failure" :severity 2))
          (with-current-buffer buffer
            (count-lines (point-min) (point-max))))
      (kill-buffer buffer))))

(qa (qa--icalendar-print-error-line-count) 1)

(princ (qa--done))
