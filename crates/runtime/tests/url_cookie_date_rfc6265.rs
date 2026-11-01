use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-cookie-rfc6265-check (issue label expected date)
    (let ((actual (url-cookie--expires-date-to-time date)))
      (unless (time-equal-p actual expected)
        (error "URL cookie RFC6265 #%s %S: %S parsed as %S, expected %S"
               issue label date actual expected))))
  (defun remacs-cookie-rfc6265-reject (issue label date)
    (let ((signaled nil))
      (condition-case nil
          (url-cookie--expires-date-to-time date)
        (error (setq signaled t)))
      (unless signaled
        (error "URL cookie RFC6265 #%s %S unexpectedly accepted %S"
               issue label date))))
  (let ((utc-2021 (encode-time 14 18 10 9 6 2021 0)))
    ;; #2339: unknown timezone words are ignored in every token position.
    (dolist (date '("Wed, 09 Jun 2021 10:18:14 GMT"
                    "PST Wed, 09 Jun 2021 10:18:14"
                    "Wed, 09 Jun PST 2021 10:18:14"
                    "Wed, 09 Jun 2021 PST 10:18:14"
                    "Wed, 09 Jun 2021 10:18:14PST"
                    "Wed, 09 Jun 2021 10:18:14 PST9"
                    "09-Jun-2021 10:18:14 GMT"
                    "Wed;09/Jun=2021?PST{10:18:14}"
                    "Wed, 09-Jun-21 10:18:14 GMT"
                    "Wed Jun 09 10:18:14 2021"
                    "10:18:14 Wed 09 2021 Jun"
                    "Wed, 09 Jun 2021 10:18:14 +0800"
                    "Wed, 09 Jun 2021 10:18:14 -0800"
                    "Wed, 09 Jun 2021 10:18:14 +08:00"))
      (remacs-cookie-rfc6265-check 2339 'timezone-token-order utc-2021 date))
    ;; Signs are delimiters, not a separately recognized timezone field.
    (remacs-cookie-rfc6265-check
     2339 'hyphen-delimiter-year (encode-time 14 18 10 9 6 2037 0)
     "Wed, 09 Jun -2037 10:18:14")
    (remacs-cookie-rfc6265-reject
     2339 'leading-offset-token-is-first-year
     "Wed, 09 Jun +0800 2037 10:18:14")
    ;; Existing URL-cookie controls remain covered.
    (remacs-cookie-rfc6265-check
     1808 'timezone-pst-control utc-2021 "Wed, 09 Jun 2021 10:18:14 PST")
    (remacs-cookie-rfc6265-check
     1808 'numeric-offset-control utc-2021 "Wed, 09 Jun 2021 10:18:14 +0800")
    (remacs-cookie-rfc6265-check
     1809 'hyphen-delimited-date utc-2021 "09-Jun-2021 10:18:14 GMT")
    (remacs-cookie-rfc6265-reject
     2339 'timezone-suffix-digit-not-a-year
     "Wed, 09 Jun PST9 10:18:14")
    ;; A compact token is consumed as day-of-month; its month/year/time suffix
    ;; cannot be rescanned as separate fields.
    (remacs-cookie-rfc6265-reject
     2339 'compact-date-is-one-token
     "Wed,09Jun2021PST10:18:14")
    (remacs-cookie-rfc6265-reject
     2339 'compact-timezone-suffix-is-one-token
     "Wed,09Jun202110:18:14PST")
    ;; #2354: month names are recognized by their three-letter prefix.
    (remacs-cookie-rfc6265-check
     2354 'month-prefix-suffix utc-2021 "Wed, 09 JunSuffix 2021 10:18:14")
    (remacs-cookie-rfc6265-check
     2354 'full-month-name-prefix utc-2021 "Wed, 09 June 2021 10:18:14")
    (remacs-cookie-rfc6265-reject
     2354 'short-month-token "Wed, 09 Ju 2021 10:18:14")
    (remacs-cookie-rfc6265-reject
     2354 'non-ascii-letter-not-ascii-month "Wed, 09 ſep 2021 10:18:14")
    ;; #2355: year matching is anchored to the token start and limited to 2--4
    ;; digits, with the required non-digit suffix boundary.
    (remacs-cookie-rfc6265-check
     2355 'year-with-nondigit-suffix utc-2021 "Wed, 09 Jun 2021PST9 10:18:14")
    (remacs-cookie-rfc6265-reject
     2355 'one-digit-year "Wed, 09 Jun 1 10:18:14")
    (remacs-cookie-rfc6265-reject
     2355 'five-digit-year "Wed, 09 Jun 20210 10:18:14")
    ;; #2351/#2376: time requires three 1--2 digit fields.
    (remacs-cookie-rfc6265-check
     2351 'one-digit-time-fields (encode-time 3 2 1 9 6 2021 0)
     "Wed, 09 Jun 2021 1:2:3")
    (remacs-cookie-rfc6265-reject
     2351 'three-digit-hour "Wed, 09 Jun 2021 001:02:03")
    (remacs-cookie-rfc6265-reject
     2351 'three-digit-minute "Wed, 09 Jun 2021 01:002:03")
    (remacs-cookie-rfc6265-reject
     2351 'three-digit-second "Wed, 09 Jun 2021 01:02:003")
    (remacs-cookie-rfc6265-reject
     2376 'seconds-required "Wed, 09 Jun 2021 10:18")
    ;; #2352: all four date fields are required.
    (remacs-cookie-rfc6265-reject 2352 'missing-time "Wed, 09 Jun 2021")
    (remacs-cookie-rfc6265-reject 2352 'missing-day "Wed, Jun 2021 10:18:14")
    (remacs-cookie-rfc6265-reject 2352 'missing-month "Wed, 09 2021 10:18:14")
    (remacs-cookie-rfc6265-reject 2352 'missing-year "Wed, 09 Jun 10:18:14")
    ;; #2348: clock bounds.
    (remacs-cookie-rfc6265-check
     2348 'last-valid-clock (encode-time 59 59 23 9 6 2021 0)
     "Wed, 09 Jun 2021 23:59:59")
    (remacs-cookie-rfc6265-reject 2348 'hour-overflow
                                  "Wed, 09 Jun 2021 24:00:00")
    (remacs-cookie-rfc6265-reject 2348 'minute-overflow
                                  "Wed, 09 Jun 2021 23:60:00")
    (remacs-cookie-rfc6265-reject 2348 'second-overflow
                                  "Wed, 09 Jun 2021 23:59:60")
    ;; #2349: year lower bound and actual Gregorian dates.
    (remacs-cookie-rfc6265-reject 2349 'year-before-1601
                                  "Wed, 09 Jun 1600 10:18:14")
    (remacs-cookie-rfc6265-check
     2349 'year-1601 (encode-time 14 18 10 9 6 1601 0)
     "Wed, 09 Jun 1601 10:18:14")
    (remacs-cookie-rfc6265-check
     2349 'leap-day (encode-time 14 18 10 29 2 2020 0)
     "Sat, 29 Feb 2020 10:18:14")
    (remacs-cookie-rfc6265-reject 2349 'nonexistent-leap-day
                                  "Mon, 29 Feb 2021 10:18:14")
    (remacs-cookie-rfc6265-reject 2349 'nonexistent-april-31
                                  "Sat, 31 Apr 2021 10:18:14")
    ;; #1865: map year values 00..69 to 2000..2069 and 70..99 to 1970..1999.
    (remacs-cookie-rfc6265-check
     1865 'year-00 (encode-time 14 18 10 9 6 2000 0)
     "Wed, 09 Jun 00 10:18:14")
    (remacs-cookie-rfc6265-check
     1865 'year-69 (encode-time 14 18 10 9 6 2069 0)
     "Wed, 09 Jun 69 10:18:14")
    (remacs-cookie-rfc6265-check
     1865 'year-70 (encode-time 14 18 10 9 6 1970 0)
     "Wed, 09 Jun 70 10:18:14")
    (remacs-cookie-rfc6265-check
     1865 'year-99 (encode-time 14 18 10 9 6 1999 0)
     "Wed, 09 Jun 99 10:18:14")
    ;; #1806: 3/4-digit zero-prefixed year tokens use their numeric value.
    (dolist (case '(("Wed, 09 Jun 050 10:18:14 GMT" . 2050)
                    ("Wed, 09 Jun 0050 10:18:14 GMT" . 2050)
                    ("Wed, 09 Jun 50 10:18:14 GMT" . 2050)
                    ("Wed, 09 Jun 069 10:18:14 GMT" . 2069)
                    ("Wed, 09 Jun 0069 10:18:14 GMT" . 2069)
                    ("Wed, 09 Jun 69 10:18:14 GMT" . 2069)))
      (remacs-cookie-rfc6265-check
       1806 'zero-prefixed-year
       (encode-time 14 18 10 9 6 (cdr case) 0)
       (car case)))
    ;; #1811: an out-of-range day remains the day field, not a year token.
    (remacs-cookie-rfc6265-reject 1811 'invalid-day-not-year
                                  "Thu, 50 Jan 10:18:14 GMT")
    (remacs-cookie-rfc6265-reject 1811 'invalid-day-with-valid-year
                                  "Wed, 32 Jun 21 10:18:14"))
  t)"###;

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

#[test]
fn url_cookie_date_parser_follows_rfc6265_511() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/url-cookie.el");
    let test_cookie_file = std::env::temp_dir().join("remacs-url-cookie-rfc6265-test");
    let code = format!(
        "(progn
         (setq noninteractive t)
         (defvar url-setup-done nil)
         (defvar url-cookie-file nil)
         (defvar url-cookie-storage nil)
         (defvar url-cookie-secure-storage nil)
         (defvar url-cookies-changed-since-last-save nil)
         (defvar url-cookie-timer nil)
         (defvar url-cookie-save-interval 0)
         (let ((url-setup-done nil)
               (url-cookie-file {})
               (url-cookie-storage nil)
               (url-cookie-secure-storage nil)
               (url-cookies-changed-since-last-save nil)
               (url-cookie-timer nil)
               (url-cookie-save-interval 0))
           (load {} nil t t)
           {}))",
        lisp_string(test_cookie_file.to_str().unwrap()),
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
}
