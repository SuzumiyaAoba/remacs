use remacs_runtime::lisp::{Flow, Interp};

const SETUP: &str = r##"
(progn
  (require 'icalendar-parser)
  (require 'icalendar-utils)
  ;; A real extension property exercises the TIME/TZID serialization contract.
  (icalendar-define-property remacs-test-tz-time "X-TEST-TZ-TIME"
    "Time fixture with a timezone parameter."
    icalendar-time
    :child-spec (:zero-or-one (icalendar-tzidparam)))
  (defun remacs-test-print-must-fail (printer value)
    (let (failed)
      (condition-case nil
          (funcall printer value)
        (icalendar-print-error (setq failed t)))
      (unless failed
        (error "A numeric UTC offset was silently printed as floating time"))))
  t)
"##;

const CASES: &str = r##"
(progn
  (dolist (offset '(3600 -18000 19800 0 1 -1))
    (let* ((time (list 0 0 12 nil nil nil nil -1 offset))
           (datetime (list 0 0 12 1 1 2026 nil -1 offset))
           (end (list 0 0 13 1 1 2026 nil -1 offset))
           (time-before (copy-sequence time))
           (date-before (copy-sequence datetime)))
      (unless (and (icalendar--decoded-time-p time)
                   (icalendar--decoded-date-time-p datetime))
        (error "Numeric offset fixtures must remain valid internal time values"))
      (remacs-test-print-must-fail #'icalendar-print-time time)
      (remacs-test-print-must-fail #'icalendar-print-date-time datetime)
      (remacs-test-print-must-fail #'icalendar-print-value-node
        (icalendar-make-ast-node 'icalendar-date-time (list :value datetime)))
      (remacs-test-print-must-fail #'icalendar-print-property-node
        (icalendar-make-property icalendar-dtstart datetime))
      (let* ((property (icalendar-make-property icalendar-dtstart datetime
                          (icalendar-tzidparam "Test/Zone")))
             (parameter (icalendar-ast-node-first-child-of 'icalendar-tzidparam property)))
        ;; A qualifier that cannot be printed must not be omitted while its
        ;; numeric-zone value is emitted as an unqualified floating value.
        (icalendar-ast-node-set-value parameter t)
        (remacs-test-print-must-fail #'icalendar-print-property-node property))
      (let ((date-property (icalendar-make-property icalendar-dtstart datetime
                             (icalendar-tzidparam "Test/Zone")))
            (time-property (icalendar-make-property remacs-test-tz-time time
                             (icalendar-tzidparam "Test/Zone")))
            (dates-property (icalendar-make-property icalendar-rdate (list datetime end)
                              (icalendar-tzidparam "Test/Zone")))
            (period-property
             (icalendar-make-property icalendar-rdate
               (list (icalendar-make-period datetime :end end))
               (icalendar-valuetypeparam 'icalendar-period)
               (icalendar-tzidparam "Test/Zone"))))
        (unless (and (equal (icalendar-print-property-node date-property)
                            "DTSTART;TZID=Test/Zone:20260101T120000\n")
                     (equal (icalendar-print-property-node time-property)
                            "X-TEST-TZ-TIME;TZID=Test/Zone:120000\n")
                     (equal (icalendar-print-property-node dates-property)
                            "RDATE;TZID=Test/Zone:20260101T120000,20260101T130000\n"))
          (error "A TZID-qualified property lost its local time representation"))
        (let ((printed (icalendar-print-property-node period-property)))
          (unless (and (string-search "VALUE=PERIOD" printed)
                       (string-search "TZID=Test/Zone" printed)
                       (string-search ":20260101T120000/20260101T130000\n" printed))
            (error "TZID context did not reach nested PERIOD date-times"))))
      ;; Qualified property output must not enable unqualified calls afterward.
      (remacs-test-print-must-fail #'icalendar-print-date-time datetime)
      (unless (and (equal time time-before) (equal datetime date-before))
        (error "Printing changed its input timezone or time fields"))))
  (dolist (zone '(nil t))
    (let ((time (list 0 0 12 nil nil nil nil -1 zone))
          (datetime (list 0 0 12 1 1 2026 nil -1 zone)))
      (unless (and (equal (icalendar-print-time time) (if zone "120000Z" "120000"))
                   (equal (icalendar-print-date-time datetime)
                          (if zone "20260101T120000Z" "20260101T120000")))
        (error "UTC or floating time output changed"))))
  ;; Normal observances already use floating local values and keep TZOFFSETFROM.
  (let* ((local (icalendar-read-date-time "20260101T120000"))
         (standard
          (icalendar-make-ast-node 'icalendar-standard nil
            (list (icalendar-make-property icalendar-dtstart local)
                  (icalendar-make-property icalendar-tzoffsetfrom 3600)
                  (icalendar-make-property icalendar-tzoffsetto 3600)))))
    (let ((printed (icalendar-print-component-node standard)))
      (unless (and (string-search "DTSTART:20260101T120000\n" printed)
                   (string-search "TZOFFSETFROM:+0100\n" printed))
        (error "Floating timezone observance output changed"))))
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
fn numeric_offsets_need_property_timezone_context_when_printed() {
    let mut interp = Interp::new();
    check(&mut interp, SETUP);
    check(&mut interp, CASES);
}
