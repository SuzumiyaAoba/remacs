mod common;

use common::{batch_interp, flow_str, on_big_stack};

#[test]
fn imap_el_loads_from_source() {
    let path = format!("{}/lisp/imap.el", env!("CARGO_MANIFEST_DIR"));
    let source = format!("(load {:?} nil t)", path);

    on_big_stack(move || {
        let (mut interp, _) = batch_interp();
        if let Err(flow) = interp.eval_str(&source) {
            panic!(
                "failed to load imap.el from source: {}",
                flow_str(&mut interp, &flow)
            );
        }
    });
}
