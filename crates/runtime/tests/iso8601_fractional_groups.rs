use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  (unless (equal (iso8601--make-regexp-groups-shy "\\(foo\\)")
                 "\\(?:foo\\)")
    (error "Capturing group was not made shy"))
  (unless (equal (iso8601--make-regexp-groups-shy "\\(?:foo\\)")
                 "\\(?:foo\\)")
    (error "Existing shy group was changed"))
  (unless (equal (iso8601--make-regexp-groups-shy "\\(?2:x\\)")
                 "\\(?:x\\)")
    (error "Explicitly numbered group header was not removed"))
  (unless (= (regexp-opt-depth
              (iso8601--make-regexp-groups-shy "\\(?2:x\\)"))
             0)
    (error "Explicitly numbered group remained capturing"))
  (unless (equal (iso8601--make-regexp-groups-shy "[[:alpha:]()]\\(x\\)")
                 "[[:alpha:]()]\\(?:x\\)")
    (error "Parentheses in a POSIX character class were changed"))
  (let* ((regexp (concat "[" (string 92) "]" "\\(x\\)"))
         (shy (iso8601--make-regexp-groups-shy regexp)))
    (unless (and (= (regexp-opt-depth shy) 0)
                 (string-match-p shy (concat (string 92) "x")))
      (error "Literal backslash in character alternative was mishandled")))
  (unless (equal (iso8601--make-regexp-groups-shy "\\[\\(x\\)")
                 "\\[\\(?:x\\)")
    (error "Escaped bracket was treated as a character class"))
  (dolist (input '("2024-01-02T03:04:05Z"
                   "2024-01-02T03:04:05.25Z"
                   "2024-01-02T03:04:05,25Z"))
    (unless (iso8601-valid-p input)
      (error "Rejected valid ISO time: %S" input)))
  (dolist (input '("2024-01-02T25:04:05Z"
                   "2024-01-02T03:60:05.25Z"
                   "2024-01-02T03:04:61,25Z"))
    (when (iso8601-valid-p input)
      (error "Accepted out-of-range ISO time: %S" input))
    (unless (condition-case nil (progn (iso8601-parse input) nil)
              (wrong-type-argument t))
      (error "Parsed out-of-range ISO time: %S" input)))
  (let ((full-time "03:04:05.25Z")
        (combined "2024-01-02T03:04:05.25Z"))
    (unless (and (iso8601--match iso8601--full-time-match full-time)
                 (equal (match-string 1 full-time) "03:04:05.25")
                 (equal (match-string 2 full-time) "Z"))
      (error "Full-time capture fields changed"))
    (unless (and (iso8601--match iso8601--combined-match combined)
                 (equal (match-string 1 combined) "2024-01-02")
                 (equal (match-string 2 combined) "03:04:05.25")
                 (equal (match-string 3 combined) "Z"))
      (error "Combined capture fields changed"))
    (unless (and (iso8601--match iso8601--time-match "03:04:05.25")
                 (equal (list (match-string 1 "03:04:05.25")
                              (match-string 2 "03:04:05.25")
                              (match-string 3 "03:04:05.25")
                              (match-string 4 "03:04:05.25"))
                        '("03" "04" "05" "25")))
      (error "Time-field captures changed")))
  (unless (iso8601--match iso8601--duration-match "P1.25Y")
    (error "Combined duration regexp no longer matches fractional units"))
  (let ((parsed (iso8601-parse "2024-01-02T03:04:05.25Z" t))
        (duration (iso8601-parse-duration "P2024-01-02T03:04:05.25Z")))
    (unless (equal (decoded-time-second parsed) '(525 . 100))
      (error "ISO parser did not preserve fractional seconds: %S"
             (decoded-time-second parsed)))
    ;; The combined-duration API uses iso8601-parse's default FORM,
    ;; which returns integral seconds.
    (unless (equal (decoded-time-second duration) 5)
      (error "Combined-duration parser changed its default second form: %S"
             (decoded-time-second duration))))
  (unless (time-equal-p
           (encode-time (parse-time-string "2024-01-02T03:04:05.25Z" t))
           (encode-time 5.25 4 3 2 1 2024 0))
    (error "parse-time-string did not preserve ISO fractional seconds"))
  (unless (time-equal-p
           (parse-iso8601-time-string "2024-01-02T03:04:05.25Z")
           (encode-time 5.25 4 3 2 1 2024 0))
    (error "parse-iso8601-time-string did not preserve fractional seconds"))
  (unless (time-equal-p
           (parse-iso8601-time-string "2024-01-02T03:04:05,25Z")
           (encode-time 5.25 4 3 2 1 2024 0))
    (error "parse-iso8601-time-string did not preserve comma fractions"))
  t)"#;

#[test]
fn iso8601_fractional_seconds_keep_shy_groups_and_parse_exactly() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let iso_source = root.join("lisp/iso8601.el").canonicalize().unwrap();
    let parse_source = root.join("lisp/parse-time.el").canonicalize().unwrap();
    let code = format!(
        "(progn (load {} nil t t) (load {} nil t t) {})",
        lisp_string(iso_source.to_str().unwrap()),
        lisp_string(parse_source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}
