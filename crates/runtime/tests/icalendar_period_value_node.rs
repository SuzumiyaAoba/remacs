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

const AMBIENT_CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (require 'icalendar-utils)
  (defun remacs-test-read-period-node (source)
    (icalendar-ast-node-value (icalendar-read-value-node 'icalendar-period source)))
  (defun remacs-test-period-is-second (period)
    (unless (and (equal (icalendar-print-date-time (icalendar-period-start period))
                        "20250102T090000")
                 (equal (icalendar-print-date-time (icalendar-period-end period))
                        "20250102T110000"))
      (error "The period reader used an ambient period")))
  (let ((one "20250101T090000/20250101T100000")
        (two "20250102T090000/20250102T110000"))
    (dolist (reader '(icalendar-read-period remacs-test-read-period-node))
      (with-temp-buffer
        (insert "prefix\n" one "\nsuffix")
        (narrow-to-region 8 (+ 8 (length one)))
        (goto-char (point-min))
        (re-search-forward (rx-to-string 'icalendar-period))
        (let ((before (match-data)) (position (point))
              (low (point-min)) (high (point-max)) (text (buffer-string)))
          (remacs-test-period-is-second (funcall reader two))
          (unless (and (equal before (match-data)) (= position (point))
                       (= low (point-min)) (= high (point-max))
                       (equal text (buffer-string)))
            (error "A string period read changed its caller's buffer or match data"))))
      (with-temp-buffer
        (insert "!")
        (string-match (rx-to-string 'icalendar-period) one)
        (let ((before (match-data)))
          (remacs-test-period-is-second (funcall reader two))
          (unless (and (equal before (match-data))
                       (equal (match-string 11 one) "20250101T090000"))
            (error "A period read replaced the caller's string match data"))))
      (with-temp-buffer
        (insert "ambient")
        (goto-char (point-min))
        (re-search-forward "\\(am\\)b")
        (let* ((before (match-data))
               (period (funcall reader "20250103T120000/PT90M"))
               (failed nil))
          (unless (and (equal (icalendar-print-date-time (icalendar-period-start period))
                              "20250103T120000")
                       (= (decoded-time-minute (icalendar-period-dur-value period)) 90)
                       (equal before (match-data)))
            (error "A duration period read depended on unrelated match data"))
          (condition-case nil
              (funcall reader "invalid-period")
            (error (setq failed t)))
          (unless (and failed (equal before (match-data)))
            (error "A failing period read did not restore caller match data")))))
    ;; The buffer parser still consumes the value and records its real bounds.
    (with-temp-buffer
      (insert two)
      (goto-char (point-min))
      (let ((node (icalendar-parse-value-node 'icalendar-period (point-max))))
        (remacs-test-period-is-second (icalendar-ast-node-value node))
        (unless (and (= (point) (point-max))
                     (eq (icalendar-ast-node-meta-get :buffer node) (current-buffer))
                     (= (icalendar-ast-node-meta-get :begin node) (point-min))
                     (= (icalendar-ast-node-meta-get :end node) (point-max)))
          (error "Buffer period parsing lost its consumed bounds")))))
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
fn period_readers_use_their_argument_and_preserve_caller_match_data() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(&mut interp, AMBIENT_CASES);
}
