use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (dolist (case '(("20250101T100000Z/20250101T110000Z"
                   "20250101T100000Z" "20250101T110000Z" nil)
                  ("20250102T090000/20250102T110000"
                   "20250102T090000" "20250102T110000" nil)
                  ("20250101T100000Z/PT1H" "20250101T100000Z" nil "PT1H")
                  ("20250102T090000/P1DT2H3M4S" "20250102T090000" nil "P1DT2H3M4S")
                  ("20250102T090000/P2W" "20250102T090000" nil "P2W")))
    (set-match-data nil)
    (let* ((source (car case))
           (node (icalendar-read-value-node 'icalendar-period source))
           (period (icalendar-ast-node-value node)))
      (unless (and (eq (icalendar-ast-node-type node) 'icalendar-period)
                   (equal period (icalendar-read-period source)))
        (error "The value-node period reader disagrees with the direct reader"))
      (unless (equal (icalendar-print-date-time (icalendar-period-start period)) (nth 1 case))
        (error "The period start did not come from its argument: %S" source))
      (if (nth 2 case)
          (unless (and (null (icalendar-period-dur-value period))
                       (equal (icalendar-print-date-time (icalendar-period-end period))
                              (nth 2 case)))
            (error "Wrong explicit period end: %S" source))
        (unless (and (null (cadr period))
                     (equal (icalendar-print-dur-value (icalendar-period-dur-value period))
                            (nth 3 case)))
          (error "Wrong period duration: %S" source)))
      (unless (null (match-data))
        (error "Reading a period changed the caller's empty match data"))))
  t)
"##;

#[test]
fn value_node_period_reader_uses_its_argument_without_match_data() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
