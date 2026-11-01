use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CHECKS: &str = r###"(progn
  (defvar remacs-parse2997-original-iso nil)
  (defvar remacs-parse2997-custom-months nil)
  (defvar remacs-parse2997-iso-calls 0)
  (defvar remacs-parse2997-iso-error nil)
  (setq remacs-parse2997-original-iso (symbol-function 'iso8601-parse))
  (defun remacs-parse2997-observe-iso (string &optional form)
    (setq remacs-parse2997-iso-calls (1+ remacs-parse2997-iso-calls))
    (condition-case condition
        (funcall remacs-parse2997-original-iso string form)
      (wrong-type-argument
       (setq remacs-parse2997-iso-error condition)
       (signal (car condition) (cdr condition)))))
  (fset 'iso8601-parse #'remacs-parse2997-observe-iso)
  (defun remacs-parse2997-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-parse2997-parse-rfc (string)
    (let ((before remacs-parse2997-iso-calls))
      (setq remacs-parse2997-iso-error nil)
      (let ((result (parse-time-string string)))
        (remacs-parse2997-assert
         (= remacs-parse2997-iso-calls (1+ before))
         (format "RFC did not try ISO parser: %S" string))
        (remacs-parse2997-assert
         (eq (car-safe remacs-parse2997-iso-error) 'wrong-type-argument)
         (format "RFC fallback hid original ISO error: %S" string))
        result)))

  ;; Unknown obsolete alpha zones are recognized at any length after a time.
  ;; The final short-time case ensures seconds are not required.
  (dolist (zone '("A" "CET" "XYZ" "AB" "EUROPE" "LONGUNKNOWNZONE"))
    (let* ((parsed (remacs-parse2997-parse-rfc
                    (format "Wed, 15 Jan 2020 16:12:21 %s" zone))))
      (remacs-parse2997-assert
       (and (equal (nth 8 parsed) 0) (equal (nth 7 parsed) -1))
       (format "Unknown zone %S was not represented as unknown zero: %S"
               zone parsed))))
  (let ((parsed (remacs-parse2997-parse-rfc
                 "Wed, 15 Jan 2020 16:12 AB")))
    (remacs-parse2997-assert
     (and (equal (nth 8 parsed) 0) (equal (nth 7 parsed) -1))
     (format "Unknown zone after HH:MM was missed: %S" parsed)))

  ;; Alpha tokens before the parsed time are not timezone evidence.
  (dolist (string '("Wedjunk, 15 Jan 2020 16:12:21"
                    "Wed, 15 Janjunk 2020 16:12:21"
                    "Wed, 15 Jan 2020 EUROPE 16:12:21"))
    (let ((parsed (remacs-parse2997-parse-rfc string)))
      (remacs-parse2997-assert
       (and (null (nth 8 parsed)) (equal (nth 7 parsed) -1))
       (format "Pre-time alpha token became a timezone: %S => %S"
               string parsed))))

  ;; Known, numeric, commented, and token-order behavior stays unchanged.
  (let ((known (remacs-parse2997-parse-rfc
                "Wed, 15 Jan 2020 16:12:21 EST"))
        (numeric (remacs-parse2997-parse-rfc
                  "Wed, 15 Jan 2020 16:12:21 -0800"))
        (later-known (remacs-parse2997-parse-rfc
                      "Wed, 15 Jan 2020 16:12:21 UNKNOWN EST"))
        (comment (remacs-parse2997-parse-rfc
                  "Wed, 15 Jan 2020 16:12:21 (EUROPE)"))
        (custom-month
         (let ((remacs-parse2997-custom-months '(("foo" . 1)))
               (parse-time-rules
                (cons '((4) remacs-parse2997-custom-months)
                      parse-time-rules)))
           (remacs-parse2997-parse-rfc
            "Wed, 15 Foo 2020 16:12:21"))))
    (remacs-parse2997-assert
     (and (= (nth 8 known) -18000) (null (nth 7 known)))
     (format "Known EST changed: %S" known))
    (remacs-parse2997-assert
     (and (= (nth 8 numeric) -28800) (= (nth 7 numeric) -1))
     (format "Numeric zone changed: %S" numeric))
    (remacs-parse2997-assert
     (and (= (nth 8 later-known) -18000) (null (nth 7 later-known)))
     (format "Unknown-before-known ordering changed: %S" later-known))
    (remacs-parse2997-assert
     (and (null (nth 8 comment)) (= (nth 7 comment) -1))
     (format "Comment content became a timezone: %S" comment))
    (remacs-parse2997-assert
     (and (= (nth 4 custom-month) 1) (null (nth 8 custom-month)))
     (format "Custom month parsing changed: %S" custom-month)))

  ;; Valid ISO input must still return through the original ISO parser.
  (let ((before remacs-parse2997-iso-calls))
    (setq remacs-parse2997-iso-error nil)
    (let ((parsed (parse-time-string "2020-01-02T03:04:05Z")))
      (remacs-parse2997-assert
       (equal (list (nth 0 parsed) (nth 1 parsed) (nth 2 parsed)
                    (nth 3 parsed) (nth 4 parsed) (nth 5 parsed)
                    (nth 6 parsed) (nth 7 parsed) (nth 8 parsed))
              '(5 4 3 2 1 2020 nil nil 0))
       (format "ISO parse changed: %S" parsed)))
    (remacs-parse2997-assert
     (= remacs-parse2997-iso-calls (1+ before))
     "ISO input did not use the original parser")
    (remacs-parse2997-assert
     (null remacs-parse2997-iso-error)
     "Valid ISO input fell through to RFC parsing"))
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

fn eval_checks(interp: &mut Interp) {
    assert_eq!(eval(interp, CHECKS), "t");
}

#[test]
fn unknown_alpha_zones_via_fresh_require() {
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, "(progn (require 'parse-time) (featurep 'parse-time))"), "t");
    eval_checks(&mut interp);
}

#[test]
fn unknown_alpha_zones_via_full_source_load() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let iso_source = root.join("lisp/iso8601.el");
    let parse_source = root.join("lisp/parse-time.el");
    let mut interp = Interp::new();
    eval(
        &mut interp,
        &format!("(load {} nil t t)", lisp_string(iso_source.to_str().unwrap())),
    );
    eval(
        &mut interp,
        &format!("(load {} nil t t)", lisp_string(parse_source.to_str().unwrap())),
    );
    eval_checks(&mut interp);
}
