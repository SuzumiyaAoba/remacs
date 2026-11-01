use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CHECKS: &str = r###"(progn
  (unless (fboundp 'timezone-parse-date)
    (error "timezone.el definitions were not loaded"))
  (defun remacs-timezone-2982-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-timezone-2982-fields (date)
    (let ((parsed (timezone-parse-date date)))
      (list (aref parsed 0) (aref parsed 1) (aref parsed 2)
            (aref parsed 3) (aref parsed 4))))

  ;; This covers the supported styles whose year is before, after, or within
  ;; the date, and keeps month/day/time/zone fields alongside the year result.
  (let ((cases
         '(("14 Apr 001 03:20:00 GMT" "1901" "4" "14" "03:20:00" "GMT")
           ("14 Apr 049 03:20:00 GMT" "1949" "4" "14" "03:20:00" "GMT")
           ("14 Apr 050 03:20:00 GMT" "1950" "4" "14" "03:20:00" "GMT")
           ("14 Apr 100 03:20:00 GMT" "2000" "4" "14" "03:20:00" "GMT")
           ("14 Apr 999 03:20:00 GMT" "2899" "4" "14" "03:20:00" "GMT")
           ("14 Apr 1 03:20:00 GMT" "2001" "4" "14" "03:20:00" "GMT")
           ("14 Apr 01 03:20:00 GMT" "2001" "4" "14" "03:20:00" "GMT")
           ("14 Apr 68 03:20:00 GMT" "2068" "4" "14" "03:20:00" "GMT")
           ("14 Apr 69 03:20:00 GMT" "1969" "4" "14" "03:20:00" "GMT")
           ("14 Apr 0001 03:20:00 GMT" "0001" "4" "14" "03:20:00" "GMT")
           ("14 Apr 001 03:20:00" "1901" "4" "14" "03:20:00" nil)
           ;; Style 2: weekday-prefixed variant of the ordinary mail date.
           ("Fri, 17 Mar 049 4:01:33 GMT" "1949" "3" "17" "4:01:33" "GMT")
           ;; Style 3: year follows the time and zone.
           ("Mon Jan 16 16:12:37 GMT 001" "1901" "1" "16" "16:12:37" "GMT")
           ;; Style 4: compact time with a named zone.
           ("6 May 001 1641-JST" "1901" "5" "6" "1641" "-JST")
           ;; Style 5: fractional input retains the existing seconds-only time field.
           ("22-AUG-001 10:59:12.82 GMT" "1901" "8" "22" "10:59:12" "GMT")
           ;; Styles 6 and 7: weekday-first years, with and without T marker.
           ("Thu, 11 Apr 16:17:12 001 MET" "1901" "4" "11" "16:17:12" "MET")
           ("Mon, 6 Jul 16:47:20 T 001 MET" "1901" "7" "6" "16:47:20" "MET")
           ;; Style 8: dashed and compact numeric dates.
           ("001-06-24 21:13:12 GMT" "1901" "06" "24" "21:13:12" "GMT")
           ("1996-06-24 21:13:12 GMT" "1996" "06" "24" "21:13:12" "GMT")
           ("1996-06-24 21:13-ZONE" "1996" "06" "24" "21:13" "-ZONE")
           ("1996-06-24T21:13:12+01:00" "1996" "06" "24" "21:13:12" "+01:00")
           ("19960624T211312" "1996" "06" "24" "211312" nil))))
    (dolist (case cases)
      (let ((actual (remacs-timezone-2982-fields (car case)))
            (expected (cdr case)))
        (remacs-timezone-2982-assert
         (equal actual expected)
         (format "Date parse changed for %S: got %S, expected %S"
                 (car case) actual expected)))))

  ;; A date with no recognized year/month retains the historical zero vector.
  (remacs-timezone-2982-assert
   (equal (remacs-timezone-2982-fields "03:20:00 GMT")
          '("0" "0" "0" "0" nil))
   "Yearless input no longer returns the default vector")
  t)"###;

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
fn timezone_parse_date_obeys_three_digit_year_rule_2982() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/timezone.el")
        .canonicalize()
        .unwrap();
    let mut interp = Interp::new();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CHECKS
    );
    assert_eq!(eval(&mut interp, &code), "t");
}
