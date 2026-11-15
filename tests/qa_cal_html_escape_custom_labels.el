;;; -*- lexical-binding: t; -*-
;;; Customized labels in generated calendar HTML (Issues #2282, #2281, #2283).

(require 'cal-html)

(qa
 (let ((calendar-day-name-array
        ["<i>SUN</i>" "Mon" "Tue" "Wed" "Thu" "Fri" "Sat"]))
   (with-temp-buffer
     (cal-html-insert-agenda-days 1 2024 nil nil)
     (let ((html (buffer-string)))
       (list (not (string-match-p "<i>SUN</i>" html))
             (if (string-match-p "&lt;i&gt;SUN&lt;/i&gt;" html) t nil)))))
 '(t t))

(qa
 (let ((cal-html-day-abbrev-array
        ["<i>Su</i>" "Mo" "Tu" "We" "Th" "Fr" "Sa"]))
   (with-temp-buffer
     (cal-html-insert-minical 1 2024)
     (let ((html (buffer-string)))
       (list (not (string-match-p "<i>Su</i>" html))
             (if (string-match-p "&lt;i&gt;Su&lt;/i&gt;" html) t nil)))))
 '(t t))

(qa
 (let ((calendar-month-name-array
        ["<em>MONTH</em>" "Feb" "Mar" "Apr" "May" "Jun"
         "Jul" "Aug" "Sep" "Oct" "Nov" "Dec"]))
   (with-temp-buffer
     (cal-html-insert-link-monthpage 1 2024)
     (let ((html (buffer-string)))
       (list (not (string-match-p "<em>MONTH</em>" html))
             (if (string-match-p "&lt;em&gt;MONTH&lt;/em&gt;" html) t nil)))))
 '(t t))

(qa
 (let ((calendar-month-name-array
        ["<em>MONTH</em>" "Feb" "Mar" "Apr" "May" "Jun"
         "Jul" "Aug" "Sep" "Oct" "Nov" "Dec"]))
   (with-temp-buffer
     (cal-html-insert-link-yearpage 1 2024)
     (let ((html (buffer-string)))
       (list (not (string-match-p "<em>MONTH</em>" html))
             (if (string-match-p "&lt;em&gt;MONTH&lt;/em&gt;" html) t nil)))))
 '(t t))

(princ (qa--done))
