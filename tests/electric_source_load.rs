mod common;

use common::{batch_interp, flow_str, on_big_stack};

#[test]
fn electric_el_loads_from_source() {
    let path = format!("{}/lisp/electric.el", env!("CARGO_MANIFEST_DIR"));
    let source = format!(
        "(progn (load {:?} nil t) (list (fboundp 'electric-layout-post-self-insert-function-1) (fboundp 'electric-layout--update-hook) (featurep 'electric)))",
        path
    );

    on_big_stack(move || {
        let (mut interp, _) = batch_interp();
        match interp.eval_str(&source) {
            Ok(value) => assert_eq!(
                interp.prin1_to_string(&value).to_unicode_lossy(),
                "(t t t)"
            ),
            Err(flow) => panic!(
                "failed to load electric.el from source: {}",
                flow_str(&mut interp, &flow)
            ),
        }
    });
}
