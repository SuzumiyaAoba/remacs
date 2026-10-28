;;; Hebrew calendar epoch round-trips (Issue #1812).

(require 'cal-hebrew)

(qa (let* ((date '(7 1 1))
           (absolute (calendar-hebrew-to-absolute date)))
      (list absolute (calendar-hebrew-from-absolute absolute)))
    '(-1373427 (7 1 1)))

(qa (calendar-hebrew-from-absolute
     (1+ (calendar-hebrew-to-absolute '(7 1 1))))
    '(7 2 1))

(qa (let* ((absolute 0)
           (date (calendar-hebrew-from-absolute absolute)))
      (calendar-hebrew-to-absolute date))
    0)

(princ (qa--done))
