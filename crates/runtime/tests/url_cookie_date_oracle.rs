use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CHECKS: &str = r####"(progn
  ;; This reference parser implements RFC 6265 §5.1.1 independently of
  ;; `date-to-time' and of the production helper under test.
  (defun remacs-cookie-rfc6265-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-cookie-rfc6265-delimiter-p (character)
    (or (= character 9)
        (<= 32 character 47)
        (<= 59 character 64)
        (<= 91 character 96)
        (<= 123 character 126)))
  (defun remacs-cookie-rfc6265-tokenize (date)
    (let ((index 0)
          (limit (length date))
          start
          tokens)
      (while (< index limit)
        (while (and (< index limit)
                    (remacs-cookie-rfc6265-delimiter-p
                     (aref date index)))
          (setq index (1+ index)))
        (setq start index)
        (while (and (< index limit)
                    (not (remacs-cookie-rfc6265-delimiter-p
                          (aref date index))))
          (setq index (1+ index)))
        (when (< start index)
          (push (substring date start index) tokens)))
      (nreverse tokens)))
  ;; Return (VALUE NEXT-INDEX) for a bounded digit prefix whose next token
  ;; character is non-digit.  Token end acts as the delimiter boundary.
  (defun remacs-cookie-rfc6265-number-prefix (token start maximum-digits)
    (let ((index start)
          (limit (length token))
          (value 0))
      (while (and (< index limit)
                  (< (- index start) maximum-digits)
                  (<= 48 (aref token index) 57))
        (setq value (+ (* value 10) (- (aref token index) 48))
              index (1+ index)))
      (when (and (> index start)
                 (or (= index limit)
                     (not (<= 48 (aref token index) 57))))
        (list value index))))
  (defun remacs-cookie-rfc6265-day-token (token)
    (let ((number (remacs-cookie-rfc6265-number-prefix token 0 2)))
      (and number (<= 1 (cadr number) 2) (car number))))
  (defun remacs-cookie-rfc6265-year-token (token)
    (let ((number (remacs-cookie-rfc6265-number-prefix token 0 4)))
      (and number (<= 2 (cadr number) 4) (car number))))
  (defun remacs-cookie-rfc6265-time-token (token)
    (let* ((hour (remacs-cookie-rfc6265-number-prefix token 0 2))
           (hour-end (and hour (cadr hour))))
      (when (and hour (< hour-end (length token))
                 (= (aref token hour-end) 58))
        (let* ((minute-start (1+ hour-end))
               (minute
                (remacs-cookie-rfc6265-number-prefix
                 token minute-start 2))
               (minute-end (and minute (cadr minute))))
          (when (and minute (< minute-end (length token))
                     (= (aref token minute-end) 58))
            (let ((second
                   (remacs-cookie-rfc6265-number-prefix
                    token (1+ minute-end) 2)))
              (when second
                (list (car hour) (car minute) (car second)))))))))
  (defun remacs-cookie-rfc6265-month-token (token)
    (when (>= (length token) 3)
      (let ((prefix (substring token 0 3)))
        ;; RFC 6265 uses the ASCII case map, not Unicode `downcase'.
        (dotimes (index 3)
          (let ((char (aref prefix index)))
            (when (and (<= 65 char) (<= char 90))
              (aset prefix index (+ char 32)))))
        (cdr (assoc prefix
                    '(("jan" . 1) ("feb" . 2) ("mar" . 3)
                      ("apr" . 4) ("may" . 5) ("jun" . 6)
                      ("jul" . 7) ("aug" . 8) ("sep" . 9)
                      ("oct" . 10) ("nov" . 11) ("dec" . 12)))))))
  (defun remacs-cookie-rfc6265-days-in-month (month year)
    (cond ((memq month '(1 3 5 7 8 10 12)) 31)
          ((memq month '(4 6 9 11)) 30)
          ((= month 2)
           (if (or (zerop (mod year 400))
                   (and (zerop (mod year 4)) (not (zerop (mod year 100)))))
               29 28))
          (t 0)))
  (defun remacs-cookie-rfc6265-parse (date)
    (unless (stringp date)
      (signal 'wrong-type-argument (list 'stringp date)))
    (let ((found-time nil)
          (found-day nil)
          (found-month nil)
          (found-year nil)
          hour minute second day month year)
      (dolist (token (remacs-cookie-rfc6265-tokenize date))
        (let ((matched nil)
              value)
          ;; RFC order is time, day-of-month, month, year.  Once a token
          ;; matches one unset field, the rest of that token is consumed.
          (when (and (not found-time)
                     (setq value (remacs-cookie-rfc6265-time-token token)))
            (setq found-time t
                  hour (nth 0 value)
                  minute (nth 1 value)
                  second (nth 2 value)
                  matched t))
          (unless matched
            (when (and (not found-day)
                       (setq value (remacs-cookie-rfc6265-day-token token)))
              (setq found-day t day value matched t)))
          (unless matched
            (when (and (not found-month)
                       (setq value (remacs-cookie-rfc6265-month-token token)))
              (setq found-month t month value matched t)))
          (unless matched
            (when (and (not found-year)
                       (setq value (remacs-cookie-rfc6265-year-token token)))
              (setq found-year t year value matched t)))))
      (unless (and found-time found-day found-month found-year)
        (error "RFC 6265 cookie-date is missing a required field: %S" date))
      (cond ((<= 0 year 69) (setq year (+ year 2000)))
            ((<= 70 year 99) (setq year (+ year 1900))))
      (unless (and (<= 1601 year)
                   (<= 1 day 31)
                   (<= 0 hour 23)
                   (<= 0 minute 59)
                   (<= 0 second 59)
                   (<= day (remacs-cookie-rfc6265-days-in-month month year)))
        (error "RFC 6265 cookie-date is out of range: %S" date))
      ;; The timestamp is constructed in UTC, independent of the host zone.
      (encode-time second minute hour day month year 0)))

  (defun remacs-cookie-rfc6265-expected (fields)
    (encode-time (nth 5 fields) (nth 4 fields) (nth 3 fields)
                 (nth 2 fields) (nth 1 fields) (nth 0 fields) 0))
  (defun remacs-cookie-rfc6265-check-valid (label date fields)
    (let* ((expected (remacs-cookie-rfc6265-expected fields))
           (reference (remacs-cookie-rfc6265-parse date))
           (actual (url-cookie--expires-date-to-time date)))
      (remacs-cookie-rfc6265-assert
       (time-equal-p reference expected)
       (format "Reference matrix mismatch %S: %S => %S expected %S"
               label date reference expected))
      (remacs-cookie-rfc6265-assert
       (time-equal-p actual expected)
       (format "Production mismatch %S: %S => %S expected UTC %S"
               label date actual expected))))
  (defun remacs-cookie-rfc6265-check-invalid (label date)
    (let ((reference-failed
           (condition-case nil
               (progn (remacs-cookie-rfc6265-parse date) nil)
             (error t)))
          (production-failed
           (condition-case nil
               (progn (url-cookie--expires-date-to-time date) nil)
             (error t))))
      (remacs-cookie-rfc6265-assert
       reference-failed
       (format "Reference unexpectedly accepted invalid case %S: %S"
               label date))
      (remacs-cookie-rfc6265-assert
       production-failed
       (format "Production accepted invalid cookie-date %S: %S"
               label date))))

  ;; The exact RFC delimiter set, including colon as non-delimiter.
  (remacs-cookie-rfc6265-assert
   (equal (remacs-cookie-rfc6265-tokenize
           "Wed\t09;Jun=2021/10:18:14 GMT")
          '("Wed" "09" "Jun" "2021" "10:18:14" "GMT"))
   "RFC delimiter tokenization changed")
  (remacs-cookie-rfc6265-assert
   (equal (remacs-cookie-rfc6265-tokenize
           "PST9 Wed, 12 Jun 2021 10:18:14")
          '("PST9" "Wed" "12" "Jun" "2021" "10:18:14"))
   "Digits and letters in one non-delimiter token were split")
  (remacs-cookie-rfc6265-assert
   (equal (remacs-cookie-rfc6265-tokenize "10:18:14PST")
          '("10:18:14PST"))
   "Colon/alpha suffix tokenization changed")

  ;; Original #2339: a separate PST token cannot change the UTC result,
  ;; whether it is before or after the time or other date fields.
  (dolist (case
           '((2339-gmt-after "Wed, 09 Jun 2021 10:18:14 GMT")
             (2339-pst-after "Wed, 09 Jun 2021 10:18:14 PST")
             (2339-pst-before-time "Wed, 09 Jun 2021 PST 10:18:14")
             (2339-pst-before-date "PST Wed, 09 Jun 2021 10:18:14")
             (2339-pst-between-month-year "Wed, 09 Jun PST 2021 10:18:14")
             (2339-cet-after-time "Wed, 09 Jun 2021 10:18:14 CET")
             (2339-mixed-token-ignored "PST9 Wed, 12 Jun 2021 10:18:14 GMT")))
    (let ((fields (if (eq (car case) '2339-mixed-token-ignored)
                      '(2021 6 12 10 18 14)
                    '(2021 6 9 10 18 14))))
      (remacs-cookie-rfc6265-check-valid (car case) (cadr case) fields)))

  ;; Token suffixes, case-insensitive month prefix, arbitrary field order,
  ;; first-recognized values, and numeric offsets after all fields.
  (dolist (case
           '((suffixes "9x JUNiper 2021year 10:18:14PST" (2021 6 9 10 18 14))
             (delimiter-punctuation "Wed\t09;Jun=2021/10:18:14 GMT"
                                    (2021 6 9 10 18 14))
             (hyphen-delimiters "09-Jun-2021 10:18:14 GMT"
                                (2021 6 9 10 18 14))
             (year-first "2021 09 Jun 10:18:14 GMT" (2021 6 9 10 18 14))
             (time-first "10:18:14 09 Jun 2021 GMT" (2021 6 9 10 18 14))
             (first-year "2021 09 Jun 10:18:14 2022 GMT"
                         (2021 6 9 10 18 14))
             (first-month "09 Jan 2021 10:18:14 Jun GMT"
                          (2021 1 9 10 18 14))
             (first-time "09 Jun 2021 10:18:14 11:22:33 GMT"
                         (2021 6 9 10 18 14))
             (first-day "09 Jun 2021 10:18:14 12 GMT"
                        (2021 6 9 10 18 14))
             (numeric-offset-ignored "Wed, 09 Jun 2021 10:18:14 +0800"
                                     (2021 6 9 10 18 14))
             (hyphen-delimited-year "09 Jun -2037 10:18:14 GMT"
                                   (2037 6 9 10 18 14))
             (no-zone-is-utc "09 Jun 2021 10:18:14" (2021 6 9 10 18 14))
             (unicode-unknown-token "日本 Wed, 09 Jun 2021 10:18:14 GMT"
                                    (2021 6 9 10 18 14))
             (valid-leap-2000 "29 Feb 2000 00:00:00 GMT" (2000 2 29 0 0 0))
             (valid-leap-2400 "29 Feb 2400 23:59:59 GMT" (2400 2 29 23 59 59))
             (earliest-year "01 Jan 1601 00:00:00 GMT" (1601 1 1 0 0 0))
             (latest-field-bounds "31 Dec 9999 23:59:59 GMT"
                                  (9999 12 31 23 59 59))
             (year-00 "01 Jan 00 00:00:00 GMT" (2000 1 1 0 0 0))
             (year-69 "01 Jan 69 00:00:00 GMT" (2069 1 1 0 0 0))
             (year-70 "01 Jan 70 00:00:00 GMT" (1970 1 1 0 0 0))
             (year-99 "01 Jan 99 00:00:00 GMT" (1999 1 1 0 0 0))
             (year-001 "09 Jun 001 10:18:14 GMT" (2001 6 9 10 18 14))
             ;; Invalid shape does not claim a field; a later valid field may.
             (invalid-month-then-valid "09 J 2021 10:18:14 Jun GMT"
                                       (2021 6 9 10 18 14))
             (invalid-year-width-then-valid "09 Jun 20370 2021 10:18:14 GMT"
                                            (2021 6 9 10 18 14))
             (one-digit-year-then-valid "09 Jun 7 2021 10:18:14 GMT"
                                        (2021 6 9 10 18 14))
             (invalid-time-shape-then-valid "09 Jun 2021 10:18 11:22:33 GMT"
                                            (2021 6 9 11 22 33))
             (invalid-time-width-then-valid "09 Jun 2021 001:02:03 10:18:14 GMT"
                                            (2021 6 9 10 18 14))))
    (remacs-cookie-rfc6265-check-valid (car case) (cadr case) (nth 2 case)))

  ;; Every issue-specific invalid input must fail instead of being normalized
  ;; by date-to-time.  A recognized but out-of-range first value is not
  ;; replaced by a later valid value.
  (dolist (case
           '((2348-hour "Wed, 09 Jun 2037 24:00:00 GMT")
             (2348-minute "Wed, 09 Jun 2037 10:60:00 GMT")
             (2348-second "Wed, 09 Jun 2037 10:18:60 GMT")
             (2348-first-invalid-hour "09 Jun 2037 24:00:00 10:18:14 GMT")
             (2349-year-1600 "09 Jun 1600 10:18:14 GMT")
             (2351-hour-three-digits "09 Jun 2037 001:02:03 GMT")
             (2351-minute-three-digits "09 Jun 2037 01:002:03 GMT")
             (2351-second-three-digits "09 Jun 2037 01:02:003 GMT")
             (2352-missing-time "09 Jun 2037 GMT")
             (2352-missing-day "Jun 2037 10:18:14 GMT")
             (2352-missing-month "09 2037 10:18:14 GMT")
             (2352-missing-year "09 Jun 10:18:14 GMT")
             (2354-invalid-month "09 J 2037 10:18:14 GMT")
             (2354-nonascii-month "09 ſep 2037 10:18:14 GMT")
             (2355-one-digit-year "09 Jun 7 10:18:14 GMT")
             (2355-five-digit-year "09 Jun 20370 10:18:14 GMT")
             (2376-missing-seconds "09 Jun 2037 10:18 GMT")
             (2376-hour-only "09 Jun 2037 10 GMT")
             (day-zero "00 Jun 2021 10:18:14 GMT")
             (first-invalid-day "32 09 Jun 2021 10:18:14 GMT")
             (first-invalid-year "1500 2021 09 Jun 10:18:14 GMT")
             (plus-offset-is-year-token "09 Jun +0800 2021 10:18:14 GMT")
             (year-below-min "09 Jun 1600 10:18:14 GMT")
             (calendar-april-31 "31 Apr 2021 10:18:14 GMT")
             (calendar-february-1900 "29 Feb 1900 10:18:14 GMT")
             (calendar-february-2021 "29 Feb 2021 10:18:14 GMT")
             (no-month "09 Jun2021 10:18:14 GMT")
             (no-year-in-alphanumeric-token "09 2021Jun 10:18:14 GMT")
             (compact-no-delimiters "Wed,09Jun2021PST10:18:14")))
    (remacs-cookie-rfc6265-check-invalid (car case) (cadr case)))

  ;; Invalid date strings are errors at the helper boundary; callers then
  ;; ignore the Expires attribute / treat the cookie as not expired.
  (let ((type-error
         (condition-case err
             (progn (url-cookie--expires-date-to-time 42) nil)
           (wrong-type-argument err)
           (error (list 'other err)))))
    (remacs-cookie-rfc6265-assert
     (and (consp type-error)
          (eq (car type-error) 'wrong-type-argument)
          (eq (cadr type-error) 'sequencep))
     (format "Non-string input lost its wrong-type-argument: %S" type-error)))
  (let ((cookie (url-cookie-create
                 :name "sid" :value "x"
                 :expires "09 Jun 1600 10:18:14 GMT"
                 :domain "example.com" :localpart "/" :secure nil)))
    (when (url-cookie-expired-p cookie)
      (error "An invalid Expires date was treated as expired")))
  (let ((cookie (url-cookie-create
                 :name "sid" :value "x"
                 :expires "Thu, 01 Jan 70 00:00:00 GMT"
                 :domain "example.com" :localpart "/" :secure nil)))
    (unless (url-cookie-expired-p cookie)
      (error "#1865 two-digit 70 year was not treated as expired")))
  t)"####;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

fn run_fixture(load_mode: u8) -> String {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let source = root.join("lisp/url-cookie.el").canonicalize().unwrap();
    let lisp_dir = root.join("lisp").canonicalize().unwrap();
    let cookie_file = std::env::temp_dir().join(format!(
        "remacs-cookie-date-{}-{}.cookie",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let load_form = if load_mode == 0 {
        format!("(load {} nil t t)", lisp_string(source.to_str().unwrap()))
    } else if load_mode == 1 {
        let compiled = root.join("lisp-elc/url-cookie.elc");
        format!("(load {} nil t t)", lisp_string(compiled.to_str().unwrap()))
    } else {
        format!(
            "(let ((load-path (cons {} load-path))) (require 'url-cookie))",
            lisp_string(lisp_dir.to_str().unwrap())
        )
    };
    let template = r####"(progn
      (defvar noninteractive nil)
      (defvar url-setup-done nil)
      (defvar url-cookie-file nil)
      (defvar url-cookie-storage nil)
      (defvar url-cookie-secure-storage nil)
      (defvar url-cookies-changed-since-last-save nil)
      (defvar url-cookie-timer nil)
      (defvar url-cookie-save-interval nil)
      (defvar remacs-cookie-rfc6265-timer-calls 0)
      (defvar remacs-cookie-rfc6265-original-run-at-time nil)
      (defvar remacs-cookie-rfc6265-had-run-at-time nil)
      (defun remacs-cookie-rfc6265-load-dependencies ()
        (dolist (feature '(url-util url-parse url-domsuf generate-lisp-file))
          (condition-case err
              (require feature)
            (error (error "Cookie fixture dependency %s failed: %S" feature err)))))
      (defun remacs-cookie-rfc6265-block-run-at-time (&rest _args)
        (setq remacs-cookie-rfc6265-timer-calls
              (1+ remacs-cookie-rfc6265-timer-calls))
        (make-symbol "blocked-cookie-timer"))
      (let ((noninteractive t)
            (load-path (cons LISP_DIR load-path))
            (url-setup-done nil)
            (url-cookie-file COOKIE_FILE)
            (url-cookie-storage nil)
            (url-cookie-secure-storage nil)
            (url-cookies-changed-since-last-save nil)
            (url-cookie-timer nil)
            (url-cookie-save-interval nil)
            (remacs-cookie-rfc6265-timer-calls 0))
        (unless (not (file-exists-p COOKIE_FILE))
          (error "Temporary cookie path unexpectedly exists"))
        (unwind-protect
            (progn
              ;; Preload the ordinary URL dependencies while cookie state is
              ;; already sandboxed, then intercept timer scheduling around
              ;; url-cookie's own load/require path.
              (remacs-cookie-rfc6265-load-dependencies)
              (setq remacs-cookie-rfc6265-had-run-at-time
                    (fboundp 'run-at-time)
                    remacs-cookie-rfc6265-original-run-at-time
                    (and remacs-cookie-rfc6265-had-run-at-time
                         (symbol-function 'run-at-time)))
              (fset 'run-at-time #'remacs-cookie-rfc6265-block-run-at-time)
              (condition-case err
                  LOAD_FORM
                (error (error "Cookie fixture library load failed: %S" err)))
              (unless (featurep 'url-cookie)
                (error "url-cookie source did not provide its feature"))
              (unless (zerop remacs-cookie-rfc6265-timer-calls)
                (error "Loading url-cookie attempted to register a timer"))
              (unless (null url-cookie-timer)
                (error "Loading url-cookie left a timer value"))
              (condition-case err
                  CHECKS
                (error (error "Cookie fixture checks failed: %S" err)))
              (unless (zerop remacs-cookie-rfc6265-timer-calls)
                (error "Date parsing attempted to register a timer"))
              (unless (null url-cookie-timer)
                (error "Date parsing left a timer value"))
              (when (file-exists-p COOKIE_FILE)
                (error "Date parsing unexpectedly created the cookie file"))
              t)
          (progn
            (if remacs-cookie-rfc6265-had-run-at-time
                (fset 'run-at-time remacs-cookie-rfc6265-original-run-at-time)
              (fmakunbound 'run-at-time))
            ;; If a regression unexpectedly wrote this isolated fixture file,
            ;; remove only the unique temporary path created for this test.
            (when (file-exists-p COOKIE_FILE)
              (delete-file COOKIE_FILE))))))"####;
    let code = template
        .replace("COOKIE_FILE", &lisp_string(cookie_file.to_str().unwrap()))
        .replace("LISP_DIR", &lisp_string(lisp_dir.to_str().unwrap()))
        .replace("LOAD_FORM", &load_form)
        .replace("CHECKS", CHECKS);
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
    "t".to_owned()
}

#[test]
fn url_cookie_date_parser_matches_rfc6265_oracle_from_source_load() {
    assert_eq!(run_fixture(0), "t");
}

#[test]
fn url_cookie_date_parser_matches_rfc6265_oracle_from_fresh_require() {
    assert_eq!(run_fixture(2), "t");
}

#[test]
fn url_cookie_date_parser_matches_rfc6265_oracle_from_compiled_load() {
    assert_eq!(run_fixture(1), "t");
}
