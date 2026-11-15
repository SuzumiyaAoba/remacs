use remacs_runtime::lisp::{Interp, Value};

#[test]
fn font_match_p_compares_families_and_accepts_entity_and_object_records() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(list\
                       (font-match-p (font-spec :family \"First Family\")\
                                     (font-spec :family \"Other Family\"))\
                       (font-match-p (font-spec :family \"Same Family\")\
                                     (font-spec :family \"Same Family\"))\
                       (font-match-p (font-spec :family \"Test Family\")\
                                     (record 'font-entity nil nil \"Test Family\"))\
                       (font-match-p (font-spec :family \"Test Family\")\
                                     (record 'font-object nil nil \"Test Family\"))\
                       (font-match-p (font-spec :family \"Test Family\" :weight 'bold)\
                                     (record 'font-entity nil nil \"Test Family\" nil nil 51200)))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "(nil t t t t)");

            let bad = interp
                .eval_str(
                    "(condition-case error\
                         (font-match-p (font-spec) 42)\
                       (wrong-type-argument error))",
                )
                .unwrap();
            assert!(matches!(bad, Value::Cons(_)));
        })
        .unwrap()
        .join()
        .unwrap();
}
