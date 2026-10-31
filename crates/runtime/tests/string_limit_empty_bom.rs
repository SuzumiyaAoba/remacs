use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(progn
  (let ((bom (encode-coding-string "" 'utf-16))
        (utf8-empty (encode-coding-string "" 'utf-8)))
    (unless (and
             (equal (string-limit "" 0 nil 'utf-16) "")
             (equal (string-limit "" 1 nil 'utf-16) "")
             (equal (string-limit "" 2 nil 'utf-16) bom)
             (equal (string-limit "" 3 nil 'utf-16) bom)
             (equal (string-limit "" 2 t 'utf-16) bom)
             (= (length bom) 2)
             (not (multibyte-string-p (string-limit "" 2 nil 'utf-16)))
             (equal (string-limit "語" 2 nil 'utf-16) bom)
             (equal (string-limit "語" 2 t 'utf-16) bom)
             (equal (string-limit "" 0 nil 'utf-8) utf8-empty))
      (error "string-limit lost or exceeded the encoded empty prefix")))
  t)"#;

fn evaluate(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn string_limit_keeps_a_fitting_empty_encoding_prefix() {
    let mut interp = Interp::new();
    evaluate(&mut interp, CASES);
    evaluate(
        &mut interp,
        "(progn (fmakunbound 'string-limit) (load \"subr-x\") t)",
    );
    evaluate(&mut interp, CASES);
}
