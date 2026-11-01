use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  ;; Load the real implementations in dependency order.
  (load ISO8601_SOURCE nil t t)
  (load PARSE_TIME_SOURCE nil t t)
  (defvar remacs-3017-iso-original nil)
  (defvar remacs-3017-iso-calls 0)
  (defvar remacs-3017-iso-last-error nil)
  (setq remacs-3017-iso-original (symbol-function 'iso8601-parse))
  (defun remacs-3017-observe-iso8601-parse (string &optional form)
    (setq remacs-3017-iso-calls (1+ remacs-3017-iso-calls))
    (condition-case condition
        (funcall remacs-3017-iso-original string form)
      (wrong-type-argument
       ;; Record and re-signal the original condition and data unchanged.
       (setq remacs-3017-iso-last-error condition)
       (signal (car condition) (cdr condition)))))
  (fset 'iso8601-parse #'remacs-3017-observe-iso8601-parse)
  ;; A valid ISO timestamp must be handled by the real parser without fallback.
  (setq remacs-3017-iso-last-error nil)
  (let* ((before remacs-3017-iso-calls)
         (parsed (parse-time-string "2020-01-01T00:00:00+01:30")))
    (unless (equal parsed '(0 0 0 1 1 2020 nil -1 5400))
      (error "ISO timestamp changed: %S" parsed))
    (unless (= remacs-3017-iso-calls (1+ before))
      (error "ISO parser call count changed"))
    (unless (null remacs-3017-iso-last-error)
      (error "Valid ISO timestamp unexpectedly signaled: %S"
             remacs-3017-iso-last-error)))
  (let ((cases '(("Wed, 1 Jan 2020 00:00:00 +0000" . (0 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 -0000" . (0 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +2300" . (82800 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 -2300" . (-82800 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +2359" . (86340 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 -2359" . (-86340 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +9960" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 -9960" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +0060" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +2360" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +00000" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +0000x" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +00x0" . (nil . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +2400" . (86400 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 -2400" . (-86400 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 +9900" . (356400 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 -9900" . (-356400 . -1))
                 ("Wed, 1 Jan 2020 00:00:00 GMT" . (0 . nil))
                 ("Wed, 1 Jan 2020 00:00:00 EST" . (-18000 . nil))
                 ("Wed, 1 Jan 2020 00:00:00 CET" . (0 . -1)))))
    (dolist (case cases)
      (let* ((string (car case))
             (expected (cdr case))
             (expected-zone (car expected))
             (expected-dst (cdr expected))
             (before remacs-3017-iso-calls))
        (setq remacs-3017-iso-last-error nil)
        (let ((parsed (parse-time-string string)))
          ;; Check all slots so a rejected zone cannot silently become an
          ;; hour/day/year token during the RFC fallback.
          (unless (equal (list (nth 0 parsed) (nth 1 parsed) (nth 2 parsed)
                               (nth 3 parsed) (nth 4 parsed) (nth 5 parsed)
                               (nth 6 parsed) (nth 7 parsed) (nth 8 parsed))
                         (list 0 0 0 1 1 2020 3 expected-dst expected-zone))
            (error "RFC zone parse mismatch for %S: %S" string parsed)))
        (unless (= remacs-3017-iso-calls (1+ before))
          (error "ISO parser was not called exactly once for %S" string))
        (unless (equal remacs-3017-iso-last-error
                       (list 'wrong-type-argument string))
          (error "RFC input did not preserve the original ISO signal for %S: %S"
                 string remacs-3017-iso-last-error)))))
  ;; Invalid ISO syntax is rejected by the real ISO parser; its original error
  ;; reaches parse-time-string, whose RFC fallback leaves the whole ISO token
  ;; uninterpreted rather than treating its zone as a separate RFC field.
  (let* ((string "2020-01-01T00:00:00+99:60")
         (before remacs-3017-iso-calls))
    (setq remacs-3017-iso-last-error nil)
    (let ((parsed (parse-time-string string)))
      (unless (equal parsed '(nil nil nil nil nil nil nil -1 nil))
        (error "Invalid ISO offset was reinterpreted: %S" parsed)))
    (unless (= remacs-3017-iso-calls (1+ before))
      (error "ISO parser was not called for invalid ISO input"))
    (unless (equal remacs-3017-iso-last-error
                   (list 'wrong-type-argument string))
      (error "Invalid ISO input did not preserve the original signal: %S"
             remacs-3017-iso-last-error))
  t))"##;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn parse_time_numeric_zone_checks_rfc5322_minute_range() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT")).canonicalize().unwrap();
    let iso = root.join("lisp/iso8601.el");
    let parse_time = root.join("lisp/parse-time.el");
    let mut interp = Interp::new();
    let fixture = CASES
        .replace("ISO8601_SOURCE", &lisp_string(iso.to_str().unwrap()))
        .replace("PARSE_TIME_SOURCE", &lisp_string(parse_time.to_str().unwrap()));
    let result = interp.eval_str(&fixture).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "t");
}
