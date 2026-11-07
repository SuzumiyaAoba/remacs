use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

fn labels(interp: &Interp, name: &str) -> Vec<String> {
    render_grid(interp, 80, 24)
        .menus
        .into_iter()
        .find(|menu| menu.label == name)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect()
}

#[test]
fn suffix_is_expanded_by_easy_menu_and_ignored_as_a_raw_property() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(include_str!("fixtures/native_menu_suffix.el"))
                .unwrap();
            for (enabled, expected) in [
                (true, ["Mode (on)", "Static fixed", "Label on +"]),
                (false, ["Mode (off)", "Static fixed", "Label off -"]),
            ] {
                interp
                    .eval_str(if enabled {
                        "(setq qa-suffix-on t)"
                    } else {
                        "(setq qa-suffix-on nil)"
                    })
                    .unwrap();
                prepare_native_menus(&mut interp).unwrap();
                assert_eq!(labels(&interp, "QA Raw Suffix"), ["Mode"]);
                assert_eq!(labels(&interp, "QA Easy Suffix"), expected);
                assert!(
                    interp
                        .eval_str("(= qa-raw-suffix-calls 0)")
                        .unwrap()
                        .truthy(),
                    "GNU does not evaluate unsupported raw menu-item properties"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
