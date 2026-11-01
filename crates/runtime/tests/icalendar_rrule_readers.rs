use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (with-temp-buffer
    (insert "COUNT=4")
    (goto-char (point-min))
    (re-search-forward (rx-to-string 'icalendar-rrule-part))
    (unless (equal (icalendar-read-rrule-part "FREQ=DAILY") '(FREQ DAILY))
      (error "The RRULE part reader used an ambient clause")))
  (let ((cases
         (append
          '(("FREQ=DAILY" (FREQ DAILY)) ("COUNT=4" (COUNT 4))
            ("INTERVAL=2" (INTERVAL 2)) ("BYSECOND=0,30,60" (BYSECOND (0 30 60)))
            ("BYMINUTE=0,30,59" (BYMINUTE (0 30 59)))
            ("BYHOUR=0,12,23" (BYHOUR (0 12 23)))
            ("BYMONTHDAY=-31,1,31" (BYMONTHDAY (-31 1 31)))
            ("BYYEARDAY=-366,1,366" (BYYEARDAY (-366 1 366)))
            ("BYWEEKNO=-53,1,53" (BYWEEKNO (-53 1 53)))
            ("BYMONTH=1,12" (BYMONTH (1 12)))
            ("BYSETPOS=-1,1" (BYSETPOS (-1 1)))
            ("BYDAY=MO,1TU,-1SU" (BYDAY (1 (2 . 1) (0 . -1))))
            ("WKST=SU" (WKST 0)))
          (list (list "UNTIL=20250101" '(UNTIL (1 1 2025)))
                (list "UNTIL=20250101T090000Z"
                      (list 'UNTIL (icalendar-read-date-time "20250101T090000Z")))))))
    (dolist (case cases)
      (dolist (context '(empty buffer string))
        (with-temp-buffer
          (cond
           ((eq context 'empty) (set-match-data nil))
           ((eq context 'buffer)
            (insert "COUNT=99")
            (goto-char (point-min))
            (re-search-forward (rx-to-string 'icalendar-rrule-part)))
           (t
            (insert "!")
            (string-match (rx-to-string 'icalendar-rrule-part) "COUNT=99")))
          (let* ((before (match-data)) (position (point))
                 (actual (icalendar-read-rrule-part (car case))))
            (unless (and (equal actual (cadr case))
                         (equal before (match-data)) (= position (point)))
              (error "Wrong RRULE part or caller state: %S in %S produced %S"
                     (car case) context actual)))))))
  (let ((source "FREQ=YEARLY;BYMONTH=1,2;BYDAY=MO,1TU;COUNT=4")
        (expected '((FREQ YEARLY) (BYMONTH (1 2)) (BYDAY (1 (2 . 1))) (COUNT 4))))
    (unless (equal (icalendar-read-rrule-value source) expected)
      (error "The complete RRULE reader lost its parsed clauses"))
    (let ((node (icalendar-read-value-node 'icalendar-rrule-value source)))
      (unless (and (eq (icalendar-ast-node-type node) 'icalendar-rrule-value)
                   (equal (icalendar-ast-node-value node) expected))
        (error "The RRULE value-node API disagrees with the complete reader"))))
  t)
"##;

const WEEKDAY_CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (with-temp-buffer
    (insert "1MO")
    (goto-char (point-min))
    (re-search-forward (rx-to-string 'icalendar-weekdaynum))
    (unless (equal (icalendar-read-weekdaynum "2TU") '(2 . 2))
      (error "The weekday reader used an ambient weekday")))
  (dolist (weekday '(("SU" 0) ("MO" 1) ("TU" 2) ("WE" 3)
                     ("TH" 4) ("FR" 5) ("SA" 6)))
    (dolist (offset '(nil 1 -1 2 -2 53 -53))
      (let* ((source (concat (if offset
                                 (concat (if (> offset 0) "+" "") (number-to-string offset))
                               "")
                             (car weekday)))
             (expected (if offset (cons (cadr weekday) offset) (cadr weekday)))
             (canonical (concat (if offset (number-to-string offset) "") (car weekday))))
        (dolist (context '(empty buffer string))
          (with-temp-buffer
            (cond
             ((eq context 'empty) (set-match-data nil))
             ((eq context 'buffer)
              (insert "1MO")
              (goto-char (point-min))
              (re-search-forward (rx-to-string 'icalendar-weekdaynum)))
             (t
              (insert "!")
              (string-match (rx-to-string 'icalendar-weekdaynum) "1MO")))
            (let* ((before (match-data)) (position (point))
                   (actual (icalendar-read-weekdaynum source)))
              (unless (and (equal actual expected)
                           (equal (icalendar-print-weekdaynum actual) canonical)
                           (equal before (match-data)) (= position (point)))
                (error "Wrong weekday or caller state: %S in %S produced %S"
                       source context actual))))))))
  ;; A string reader can also be mapped without pre-matching each item.
  (set-match-data nil)
  (unless (equal (icalendar-read-list-with #'icalendar-read-weekdaynum
                                          "MO,2TU,-1SU" nil ",")
                 '(1 (2 . 2) (0 . -1)))
    (error "Unseeded weekday list reads did not use each input string"))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn rrule_and_weekday_readers_use_their_arguments_and_preserve_match_data() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, WEEKDAY_CASES);
}
