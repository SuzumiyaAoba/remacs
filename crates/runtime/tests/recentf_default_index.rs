use remacs_runtime::lisp::{Flow, Interp};

#[test]
fn recentf_open_most_recent_file_defaults_to_first_entry() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"(progn
             (require 'recentf)
             (let ((recentf-list '("one" "two" "three" "four" "five"
                                    "six" "seven" "eight" "nine" "ten"))
                   (recentf--files-with-key nil)
                   (recentf-menu-action #'identity))
               (list
                (recentf-open-most-recent-file)
                (recentf-open-most-recent-file nil)
                (recentf-open-most-recent-file 1)
                (recentf-open-most-recent-file 2)
                (recentf-open-most-recent-file 0)
                (let ((current-prefix-arg 2))
                  (call-interactively 'recentf-open-most-recent-file))
                (mapcar (lambda (n)
                          (condition-case nil
                              (recentf-open-most-recent-file n)
                            (error 'error)))
                        '(10 -1))
                (let ((recentf-list nil))
                  (condition-case nil
                      (progn (recentf-open-most-recent-file) 'no-error)
                    (error 'error))))))"#,
        )
        .unwrap_or_else(|flow| match flow {
            Flow::Signal(signal) => panic!(
                "{} {}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data())
            ),
            other => panic!("{other:?}"),
        });
    assert_eq!(
        interp.prin1_to_string(&result),
        "(\"one\" \"one\" \"one\" \"two\" \"ten\" \"two\" (error error) error)"
    );
}
