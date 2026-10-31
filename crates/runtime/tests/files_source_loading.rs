use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn loading_files_source_defines_forms_after_cd_and_auto_mode() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/files.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t) (list (fboundp 'cd) (fboundp 'set-auto-mode--find-matching-alist-entry) (fboundp 'file-attribute-collect)))",
        lisp_string(source.to_str().unwrap())
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t)");
}
