;;; -*- lexical-binding: nil; -*-
;; Native process inspection and checked PID conversion (#607/#608).
(dolist (value '(nil t "1" (1 . 2) [1]))
  (qa-err-data (process-attributes value) 'wrong-type-argument (list 'numberp value)))
(let ((marker (point-marker)))
  (qa-err-data (process-attributes marker) 'wrong-type-argument (list 'numberp marker)))
(let ((process (make-pipe-process :name "qa608-type" :noquery t)))
  (unwind-protect
      (qa-err-data (process-attributes process) 'wrong-type-argument (list 'numberp process))
    (delete-process process)))
(dolist (value '(1.5 -1.5 0.0e+NaN 1.0e+INF -1.0e+INF
                 2147483648 -2147483649 4294967297
                 2305843009213693952 1000000000000000000000000000000000000000000))
  (qa-err-data (process-attributes value) 'error
               '("Not an in-range integer, integral float, or cons of integers")))
(qa (process-attributes -1) nil)
(qa (process-attributes -1.0) nil)
(qa (process-attributes 2147483647) nil)
;; Oversized PIDs must not alias the current process through a 32-bit cast.
(qa-err-data (process-attributes (+ (expt 2 32) (emacs-pid))) 'error
             '("Not an in-range integer, integral float, or cons of integers"))
(let ((attributes (process-attributes (emacs-pid))))
  (qa (not (null attributes)) t)
  (qa (cdr (assq 'euid attributes)) (user-uid))
  (qa (integerp (cdr (assq 'egid attributes))) t)
  (qa (stringp (cdr (assq 'comm attributes))) t)
  (qa (stringp (cdr (assq 'args attributes))) t)
  (qa (stringp (cdr (assq 'state attributes))) t)
  (qa (integerp (cdr (assq 'ppid attributes))) t)
  (qa (integerp (cdr (assq 'pgrp attributes))) t)
  (qa (integerp (cdr (assq 'tpgid attributes))) t)
  (qa (integerp (cdr (assq 'nice attributes))) t)
  (qa (> (cdr (assq 'thcount attributes)) 0) t)
  (qa (> (cdr (assq 'rss attributes)) 0) t)
  (qa (>= (cdr (assq 'vsize attributes)) (cdr (assq 'rss attributes))) t)
  (qa (time-equal-p (cdr (assq 'time attributes))
                    (time-add (cdr (assq 'utime attributes)) (cdr (assq 'stime attributes)))) t)
  (qa (time-less-p (cdr (assq 'start attributes)) (current-time)) t)
  (qa (>= (float-time (cdr (assq 'etime attributes))) 0) t)
  (qa (mapcar #'car (process-attributes (float (emacs-pid)))) (mapcar #'car attributes)))
;; Native Darwin queries also handle PID 0 without requiring proc_pidinfo access.
(when (eq system-type 'darwin)
  (qa (not (null (process-attributes 0))) t)
  (qa (mapcar #'car (process-attributes 0.0)) (mapcar #'car (process-attributes 0))))
(princ (qa--done))
