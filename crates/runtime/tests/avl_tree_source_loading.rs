use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn loading_avl_tree_source_reaches_its_provide_and_iterator_definition() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/avl-tree.el");
    let quoted = lisp_string(source.to_str().unwrap());
    let code = format!(
        "(progn (load {quoted} nil t) (list (featurep 'avl-tree) (fboundp 'avl-tree-iter)))"
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
    assert_eq!(interp.prin1_to_string(&result), "(t t)");
}
