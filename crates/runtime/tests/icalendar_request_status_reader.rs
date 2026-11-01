use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (with-temp-buffer
    (insert "2.0;Success")
    (goto-char (point-min))
    (re-search-forward (rx-to-string 'icalendar-req-status-info))
    (unless (equal (icalendar-read-req-status-info "3.7;Failure") '("3.7" "Failure" nil))
      (error "The request-status reader used an ambient status")))
  (defun remacs-test-read-status-node (source)
    (let ((node (icalendar-read-value-node 'icalendar-req-status-info source)))
      (unless (eq (icalendar-ast-node-type node) 'icalendar-req-status-info)
        (error "Wrong request-status value-node type"))
      (icalendar-ast-node-value node)))
  (dolist (reader '(icalendar-read-req-status-info remacs-test-read-status-node))
    (dolist (case '(("3.7;Failure" ("3.7" "Failure" nil))
                    ("2.0;Success;details" ("2.0" "Success" "details"))
                    ("3.7;Failure;" ("3.7" "Failure" ""))
                    ("1.2.3;Pending" ("1.2.3" "Pending" nil))
                    ("3.7;Bad\\; input;bad\\,x" ("3.7" "Bad; input" "bad,x"))
                    ("4.0;Line\\nnext;C:\\\\tmp" ("4.0" "Line\nnext" "C:\\tmp"))
                    ("3.7;失敗;詳細" ("3.7" "失敗" "詳細"))
                    ("2.0;" ("2.0" "" nil))))
      (dolist (context '(empty buffer string))
        (with-temp-buffer
          (cond
           ((eq context 'empty) (set-match-data nil))
           ((eq context 'buffer)
            (insert "2.0;Success")
            (goto-char (point-min))
            (re-search-forward (rx-to-string 'icalendar-req-status-info)))
           (t
            (insert "!")
            (string-match (rx-to-string 'icalendar-req-status-info) "2.0;Success")))
          (let* ((before (match-data)) (position (point))
                 (actual (funcall reader (car case))))
            (unless (and (equal actual (cadr case))
                         (equal before (match-data)) (= position (point)))
              (error "Wrong request status or caller state: %S in %S produced %S"
                     (car case) context actual))
            ;; The text printer performs its own regexp operations, so check
            ;; the reader's preserved match data before this round trip.
            (unless (equal (icalendar-print-req-status-info actual) (car case))
              (error "Request-status text did not round trip: %S" (car case)))))))
    (with-temp-buffer
      (insert "2.0;Success")
      (goto-char (point-min))
      (re-search-forward (rx-to-string 'icalendar-req-status-info))
      (let ((before (match-data)) (failed nil))
        (condition-case nil
            (funcall reader 17)
          (error (setq failed t)))
        (unless (and failed (equal before (match-data)))
          (error "Request-status failure state: signaled=%S before=%S after=%S"
                 failed before (match-data))))))
  (with-temp-buffer
    (insert "3.7;Failure;details")
    (goto-char (point-min))
    (let ((node (icalendar-parse-value-node 'icalendar-req-status-info (point-max))))
      (unless (and (equal (icalendar-ast-node-value node) '("3.7" "Failure" "details"))
                   (= (point) (point-max))
                   (eq (icalendar-ast-node-meta-get :buffer node) (current-buffer))
                   (= (icalendar-ast-node-meta-get :begin node) (point-min))
                   (= (icalendar-ast-node-meta-get :end node) (point-max)))
        (error "Buffer request-status parsing lost its value or consumed bounds"))))
  t)
"##;

#[test]
fn request_status_readers_use_their_argument_and_preserve_match_data() {
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
