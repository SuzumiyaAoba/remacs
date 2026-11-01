use remacs_runtime::{
    frontend::{prepare_native_menus, render_grid},
    lisp::Interp,
};

fn rows(interp: &Interp) -> Vec<(String, Option<String>)> {
    let grid = render_grid(interp, 80, 24);
    grid.menus
        .into_iter()
        .find(|menu| menu.label == "QA Labels")
        .unwrap()
        .items
        .into_iter()
        .map(|item| {
            assert!(!item.separator, "ordinary headings must remain text");
            assert!(!item.checked);
            (item.label, item.command)
        })
        .collect()
}

#[test]
fn native_menu_keeps_nonselectable_labels_and_their_order_while_updating() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap))
          (let ((map (make-sparse-keymap)))
            (define-key map [section] '(menu-item "Section title"))
            (define-key map [run] '(menu-item "Run" ignore))
            (define-key global-map [menu-bar qa-labels] (cons "QA Labels" map)))
          (menu-bar-mode 1))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            let actual = rows(&interp);
            assert_eq!(actual.len(), 2);
            assert!(actual.contains(&("Section title".into(), None)));
            assert!(actual.contains(&("Run".into(), Some("ignore".into()))));

            // An explicit keymap order is retained for headings and commands.
            interp
                .eval_str(
                    r#"(progn
          (setq qa-label-value "Notes" qa-label-visible t
                qa-label-map
                '(keymap
                  (section menu-item "Section title")
                  (description menu-item "Description" nil)
                  (dynamic menu-item (concat "Status: " qa-label-value))
                  (legacy "Legacy heading")
                  (word menu-item "separator options")
                  (optional menu-item "Optional" nil :visible qa-label-visible)
                  (run menu-item "Run" ignore)))
          (define-key global-map [menu-bar qa-labels] (cons "QA Labels" qa-label-map)))"#,
                )
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(
                rows(&interp),
                [
                    ("Section title".into(), None),
                    ("Description".into(), None),
                    ("Status: Notes".into(), None),
                    ("Legacy heading".into(), None),
                    ("separator options".into(), None),
                    ("Optional".into(), None),
                    ("Run".into(), Some("ignore".into())),
                ]
            );

            interp
                .eval_str("(setq qa-label-value \"Updated\" qa-label-visible nil)")
                .unwrap();
            prepare_native_menus(&mut interp).unwrap();
            assert_eq!(
                rows(&interp),
                [
                    ("Section title".into(), None),
                    ("Description".into(), None),
                    ("Status: Updated".into(), None),
                    ("Legacy heading".into(), None),
                    ("separator options".into(), None),
                    ("Run".into(), Some("ignore".into())),
                ]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
