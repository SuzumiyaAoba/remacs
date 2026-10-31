use remacs_runtime::{
    frontend::render_grid,
    lisp::{Interp, Value},
};
use std::rc::Rc;

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn rendered(interp: &mut Interp, menu: &str) -> Vec<(String, Option<Value>)> {
    eval(interp, &format!("(x-popup-menu t {menu})"));
    let menu = interp.gui_popup_menu.clone().unwrap();
    let grid = render_grid(interp, 80, 24);
    grid.popup_menu_items
        .iter()
        .map(|item| {
            assert_eq!(item.menu_id, menu.id);
            // Pane values are returned to Lisp, including values naming commands.
            assert!(item.command.is_none());
            let label = grid.rows[item.row]
                .chars()
                .skip(item.left)
                .take(item.width)
                .collect::<String>()
                .trim()
                .to_owned();
            let value = item
                .selection
                .map(|index| menu.entries[index].value.clone().unwrap());
            (label, value)
        })
        .collect()
}

fn descriptions(
    interp: &mut Interp,
    rows: &[(String, Option<Value>)],
) -> Vec<(String, Option<String>)> {
    rows.iter()
        .map(|(label, value)| {
            (
                label.clone(),
                value
                    .as_ref()
                    .map(|value| interp.prin1_to_string(value).to_unicode_lossy()),
            )
        })
        .collect()
}

fn without_separators(rows: &[(String, Option<Value>)]) -> Vec<(String, Option<Value>)> {
    rows.iter()
        .filter(|(label, _)| label.is_empty() || !label.chars().all(|ch| ch == '─'))
        .cloned()
        .collect()
}

#[test]
fn old_fashioned_panes_render_choices_and_nonselectable_rows_in_order() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                "(progn
                   (set-frame-parameter nil 'window-system 'ns)
                   (set-frame-parameter nil 'tool-bar-lines 0)
                   (set-frame-parameter nil 'tab-bar-lines 0)
                   (set-frame-size nil 80 24))",
            );
            let rows = rendered(
                &mut interp,
                r#"'("Menu" ("Pane" ("First" . first) ("Second" . second)))"#,
            );
            assert_eq!(
                descriptions(&mut interp, &without_separators(&rows)),
                [
                    ("Menu".into(), None),
                    ("Pane".into(), None),
                    ("First".into(), Some("first".into())),
                    ("Second".into(), Some("second".into())),
                ]
            );

            eval(&mut interp, "(setq qa-pane-original (list 'original))");
            let rows = rendered(
                &mut interp,
                r#"(list "Menu"
                     (list "Left" "Read this:" '("Nil" . nil)
                           (cons "Original" qa-pane-original))
                     '("Right" ("Command value" . ignore) "After"))"#,
            );
            let separators = rows
                .iter()
                .filter(|(label, _)| !label.is_empty() && label.chars().all(|ch| ch == '─'))
                .collect::<Vec<_>>();
            assert_eq!(separators.len(), 2);
            assert!(separators[0].1.is_none());
            let content = without_separators(&rows);
            assert_eq!(
                descriptions(&mut interp, &content),
                [
                    ("Menu".into(), None),
                    ("Left".into(), None),
                    ("Read this:".into(), None),
                    ("Nil".into(), Some("nil".into())),
                    ("Original".into(), Some("(original)".into())),
                    ("Right".into(), None),
                    ("Command value".into(), Some("ignore".into())),
                    ("After".into(), None),
                ]
            );
            let original = eval(&mut interp, "qa-pane-original");
            let selected = content[4].1.clone().unwrap();
            let (Value::Cons(original), Value::Cons(selected)) = (original, selected) else {
                panic!("the original cons value must remain a cons");
            };
            assert!(Rc::ptr_eq(&original, &selected));
        })
        .unwrap()
        .join()
        .unwrap();
}
