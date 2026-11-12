use remacs_runtime::lisp::{Flow, Interp, Value};

fn menu_label(interp: &Interp, submenu: &Value) -> String {
    let label = match submenu {
        Value::Vec(vector) => vector.borrow().first().cloned().unwrap(),
        Value::Cons(wrapper) => {
            wrapper.borrow().car.clone()
        }
        _ => panic!("submenu entry"),
    };
    match label {
        Value::Str(_) => interp.prin1_to_string(&label).to_unicode_lossy(),
        _ => panic!("submenu label string"),
    }
}

#[test]
fn long_filesets_submenus_preserve_order_and_bound_labels() {
    let mut interp = Interp::new();
    match interp.eval_str("(require 'filesets)") {
        Ok(_) => {}
        Err(Flow::Signal(signal)) => panic!(
            "loading filesets failed: {}",
            interp.prin1_to_string(&signal.condition).to_unicode_lossy()
        ),
        Err(flow) => panic!("loading filesets failed: {flow:?}"),
    }
    let diagnostics = match interp.eval_str(
        r#"(let ((filesets-max-submenu-length 4)
               (filesets-max-entry-length 10)
               (filesets-menu-shortcuts-flag nil))
           (let* ((body (list (vector "Entry 00" 'ignore) (vector "Entry 01" 'ignore)
                              (vector "Entry 02" 'ignore) (vector "Entry 03" 'ignore)
                              (vector "Entry 04" 'ignore) (vector "Entry 05" 'ignore)
                              (vector "Entry 06" 'ignore) (vector "Entry 07" 'ignore)
                              (vector "Entry 08" 'ignore) (vector "Entry 09" 'ignore)))
                  (submenus (filesets-wrap-submenu body)))
             (list (length body)
                   (length (seq-subseq body 0 3))
                   (length submenus)
                   submenus)))"#,
    ) {
        Ok(result) => result,
        Err(Flow::Signal(signal)) => panic!(
            "wrapping filesets failed: {}",
            interp.prin1_to_string(&signal.condition).to_unicode_lossy()
        ),
        Err(flow) => panic!("wrapping filesets failed: {flow:?}"),
    };
    let diagnostics = diagnostics.list_to_vec().unwrap();
    assert_eq!(diagnostics[0].int(), Some(10), "submenu input length");
    assert_eq!(diagnostics[1].int(), Some(3), "first submenu slice length");
    assert_eq!(diagnostics[2].int(), Some(4), "wrapped submenu count");
    let submenus = diagnostics[3].list_to_vec().unwrap();
    assert_eq!(submenus.len(), 4);

    let first_items = submenus[0].list_to_vec().unwrap();
    assert_eq!(first_items.len(), 4);
    let first_name = menu_label(&interp, &submenus[0]);
    assert!(first_name.starts_with("\"Entry 00"), "{first_name}");
    assert!(first_name.ends_with("...\""), "{first_name}");

    assert_eq!(menu_label(&interp, &submenus[3]), "\"Entry 09\"");
}
