use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn parse_time_string_maps_unknown_obsolete_zones_to_zero() {
    let mut interp = Interp::new();
    let result = interp.eval_str(
        r#"(progn
             (require 'parse-time)
             (defvar remacs-parse-time-custom-months nil)
             (let ((military (parse-time-string
                              "Wed, 15 Jan 2020 16:12:21 A"))
                   (unknown (parse-time-string
                             "Wed, 15 Jan 2020 16:12:21 CET"))
                   (two-letters (parse-time-string
                                 "Wed, 15 Jan 2020 16:12:21 AB"))
                   (six-letters (parse-time-string
                                 "Wed, 15 Jan 2020 16:12:21 EUROPE"))
                   (unicode (parse-time-string
                             "Wed, 15 Jan 2020 16:12:21 東京"))
                   (known (parse-time-string
                           "Wed, 15 Jan 2020 16:12:21 EST"))
                   (numeric (parse-time-string
                             "Wed, 15 Jan 2020 16:12:21 -0800"))
                   (unknown-before-known (parse-time-string
                                          "Wed, 15 Jan 2020 16:12:21 FOO EST"))
                   (comment-before-numeric (parse-time-string
                                            "Wed, 15 Jan 2020 16:12:21 (CET) -0800"))
                   (comment-only (parse-time-string
                                  "Wed, 15 Jan 2020 16:12:21 (CET)"))
                   (comment-after-unknown (parse-time-string
                                           "Wed, 15 Jan 2020 16:12:21 CET (note)"))
                   (custom-month
                    (let ((remacs-parse-time-custom-months '(("foo" . 1)))
                          (parse-time-rules
                           (cons '((4) remacs-parse-time-custom-months)
                                 parse-time-rules)))
                      (parse-time-string "Wed, 15 Foo 2020 16:12:21")))
                   (missing (parse-time-string
                             "Wed, 15 Jan 2020 16:12:21")))
               (list
                (list (nth 8 military) (nth 6 military) (nth 4 military))
                (list (nth 8 unknown) (nth 6 unknown) (nth 4 unknown))
                (nth 8 two-letters)
                (nth 8 six-letters)
                (nth 8 unicode)
                (list (nth 8 known) (nth 6 known) (nth 4 known))
                (list (nth 8 numeric) (nth 6 numeric) (nth 4 numeric))
                (list (nth 8 unknown-before-known)
                      (nth 6 unknown-before-known) (nth 4 unknown-before-known))
                (list (nth 8 comment-before-numeric)
                      (nth 6 comment-before-numeric) (nth 4 comment-before-numeric))
                (nth 8 comment-only)
                (nth 8 comment-after-unknown)
                (list (nth 4 custom-month) (nth 8 custom-month))
                (list (nth 8 missing) (nth 6 missing) (nth 4 missing)))))"#,
    );
    let result = result.unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(
        interp.prin1_to_string(&result),
        "((0 3 1) (0 3 1) nil nil nil (-18000 3 1) (-28800 3 1) (-18000 3 1) (-28800 3 1) nil 0 (1 nil) (nil 3 1))"
    );
}
