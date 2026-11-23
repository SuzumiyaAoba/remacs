use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

#[test]
fn sgml_mode_fontifies_dtd_attlist_attribute_names() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/sgml-mode.el");
            let source = std::fs::read_to_string(&source).unwrap();
            let start = source
                .find("(defconst sgml--dtd-attribute-type-re")
                .unwrap();
            let end = source.find("(defconst sgml-font-lock-keywords-2").unwrap();
            let definitions = &source[start..end];

            let expression = format!(
                r#"(progn
                  (setq hack-read-symbol-shorthands-function nil
                        sgml-name-re "[_:[:alpha:]][-_.:[:alnum:]]*"
                        sgml-namespace-re "[_[:alpha:]][-_.[:alnum:]]*")
                  {definitions}
                  (with-temp-buffer
                    (insert "<!ATTLIST img\n  src CDATA #IMPLIED\n  alt (small | large) #IMPLIED\n"
                            "  title CDATA \"literal fake CDATA\">\n"
                            "<!-- ignored CDATA -->\n"
                            "<div unrelated CDATA #IMPLIED>\n")
                    (goto-char (point-min))
                    (let (attributes)
                      (while (sgml-font-lock-dtd-attribute (point-max))
                        (push (match-string-no-properties 1) attributes))
                      (list (and (assq 'sgml-font-lock-dtd-attribute
                                       sgml-font-lock-keywords-1)
                                 t)
                            (nreverse attributes)))))"#
            );

            let mut interp = Interp::new();
            let value = match interp.eval_str(&expression) {
                Ok(value) => value,
                Err(Flow::Signal(signal)) => panic!(
                    "SGML DTD fontification failed: {} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                Err(flow) => panic!("SGML DTD fontification failed: {flow:?}"),
            };
            assert_eq!(
                interp.prin1_to_string(&value).to_unicode_lossy(),
                "(t (\"src\" \"alt\" \"title\"))"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
