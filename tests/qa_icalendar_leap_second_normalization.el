;;; qa_icalendar_leap_second_normalization.el --- normalize unsupported leap seconds -*- lexical-binding: t; -*-

(require 'icalendar-parser)
(require 'icalendar-utils)

(qa
 (let* ((date-time (icalendar-read-date-time "20250101T000060Z"))
        (time (icalendar-read-time "000060Z"))
        (decoded (decode-time (encode-time date-time) t))
        (invalid-decoded-time
         (make-decoded-time :second 60 :minute 0 :hour 0
                            :day 1 :month 1 :year 2025)))
   (list (decoded-time-second date-time)
         (decoded-time-second time)
         (list (decoded-time-second decoded)
               (decoded-time-minute decoded)
               (decoded-time-hour decoded))
         (icalendar--decoded-date-time-p invalid-decoded-time)))
 '(59 59 (59 0 0) nil))

(princ (qa--done))
