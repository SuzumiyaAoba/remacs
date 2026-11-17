;;; qa_icalendar_format_error_short_name.el --- handle short buffer names -*- lexical-binding: t; -*-

(require 'icalendar)

(defun qa--icalendar-format-error-for-buffer-name (name)
  (let ((buffer (generate-new-buffer name)))
    (unwind-protect
        (stringp (icalendar-format-error :buffer buffer :message "test"))
      (kill-buffer buffer))))

(qa (qa--icalendar-format-error-for-buffer-name "qa1234567") t)
(qa (qa--icalendar-format-error-for-buffer-name "qa12345678") t)
(qa (stringp (icalendar-format-error
              :buffer (get-buffer-create "*scratch*") :message "test"))
    t)

(princ (qa--done))
