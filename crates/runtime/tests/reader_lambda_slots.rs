use remacs_runtime::lisp::Interp;

#[test]
fn reader_fixups_are_observed_by_interactive_form_for_dumped_bytecode() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.loading_dumped = true;

            let result = interp
                .eval_str(
                    r##"(let ((read-circle t))
                         (let ((function
                                (read "#1=#[0 \"\\300\\207\" [42] 1 nil #1#]")))
                           (list (eq function (aref function 5))
                                 (eq (cadr (interactive-form function)) function))))"##,
                )
                .expect("reader closure with a numbered interactive slot");

            assert_eq!(interp.prin1_to_string(&result).to_unicode_lossy(), "(t t)");
        })
        .unwrap()
        .join()
        .unwrap();
}
