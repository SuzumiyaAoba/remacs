(require 'nndiary)

(let* ((time-zones (cadr (assoc "Time-Zone" nndiary-headers)))
       (zone (nndiary-parse-schedule-value "America/New_York" time-zones nil))
       (zone-name (car zone))
       ;; The event is after the 2027 spring DST transition in New York.
       (schedule (list '(0) '(9) '(15) '(3) '(2027) nil zone))
       (now (encode-time 0 0 9 15 2 2027 zone-name))
       (next (nndiary-next-occurrence schedule now))
       (last (nndiary-last-occurrence schedule))
       (before (decode-time now zone-name))
       (next-local (decode-time next zone-name))
       (last-local (decode-time last zone-name)))
  (qa (nndiary-parse-schedule-value "EST" time-zones nil) '(-18000))
  (qa (list zone-name
            (list (nth 2 before) (nth 8 before))
            (list (nth 2 next-local) (nth 3 next-local)
                  (nth 4 next-local) (nth 5 next-local) (nth 8 next-local))
            (list (nth 2 last-local) (nth 3 last-local)
                  (nth 4 last-local) (nth 5 last-local) (nth 8 last-local)))
      '("America/New_York"
        (9 -18000)
        (9 15 3 2027 -14400)
        (9 15 3 2027 -14400)))
  (qa (condition-case err
          (nndiary-parse-schedule-value "/etc/localtime" time-zones nil)
        (error (car err)))
      'error))

(princ (qa--done))
