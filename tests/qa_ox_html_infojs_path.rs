#![recursion_limit = "4096"]
mod common;

use common::*;

#[test]
fn qa_ox_html_infojs_path() {
    let file = "qa_ox_html_infojs_path.el";
    let src = format!(
        "(progn (load {:?}) (setq qa--remacs t) (load {:?}) nil)",
        format!("{}/tests/qa_framework.el", env!("CARGO_MANIFEST_DIR")),
        format!("{}/tests/{}", env!("CARGO_MANIFEST_DIR"), file)
    );
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let (mut i, out) = batch_interp();
            match i.eval_str(&src) {
                Ok(_) => {
                    let o = out.borrow();
                    assert!(o.contains("checks passed"), "qa {file} summary: {o}");
                }
                Err(flow) => panic!(
                    "qa {file} eval error: {}; captured output:\n{}",
                    common::flow_str(&mut i, &flow),
                    out.borrow()
                ),
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
