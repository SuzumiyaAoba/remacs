//! Focused regression runner for Unicode ERC log persistence.
#![recursion_limit = "4096"]

mod common;

use common::*;

#[test]
fn qa_erc_log_unicode() {
    let src = format!(
        "(progn (load {:?}) (load {:?}) nil)",
        format!("{}/tests/qa_framework.el", env!("CARGO_MANIFEST_DIR")),
        format!("{}/tests/qa_erc_log_unicode.el", env!("CARGO_MANIFEST_DIR"))
    );
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let (mut interp, output) = batch_interp();
            match interp.eval_str(&src) {
                Ok(_) => assert!(
                    output.borrow().contains("checks passed"),
                    "qa_erc_log_unicode summary: {}",
                    output.borrow()
                ),
                Err(flow) => panic!(
                    "qa_erc_log_unicode eval error: {}; captured output:\n{}",
                    common::flow_str(&mut interp, &flow),
                    output.borrow()
                ),
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
