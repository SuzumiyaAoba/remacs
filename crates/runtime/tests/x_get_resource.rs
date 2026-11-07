use remacs_runtime::{
    frontend::XResourceBackend,
    lisp::{Flow, Interp, Value},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Default)]
struct Resources {
    values: RefCell<HashMap<(String, String), String>>,
    requests: RefCell<Vec<(String, String)>>,
}

impl XResourceBackend for Resources {
    fn get_string_resource(&self, name: &str, class: &str) -> Result<Option<String>, String> {
        self.requests
            .borrow_mut()
            .push((name.to_owned(), class.to_owned()));
        Ok(self
            .values
            .borrow()
            .get(&(name.to_owned(), class.to_owned()))
            .cloned())
    }
}

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn printed(i: &mut Interp, source: &str) -> String {
    let value = eval(i, source);
    i.prin1_to_string(&value).to_unicode_lossy()
}

#[test]
fn x_get_resource_checks_display_arguments_and_queries_gui_backend_keys() {
    let mut i = Interp::new();
    let no_display = i.eval_str(r#"(x-get-resource nil nil)"#);
    assert!(matches!(no_display, Err(Flow::Signal(_))));

    eval(
        &mut i,
        "(progn (set-frame-parameter nil 'window-system 'w32) (setq inhibit-x-resources nil))",
    );
    let app_name = match eval(&mut i, "invocation-name") {
        Value::Str(value) => value.borrow().to_unicode_lossy(),
        value => panic!("invocation-name: {}", i.prin1_to_string(&value)),
    };
    let simple_name = format!("{app_name}.menuBar");
    let component_name = format!("{app_name}.frame.menuBar");
    let resources = Rc::new(Resources::default());
    resources.values.borrow_mut().insert(
        (simple_name.clone(), "Emacs.MenuBar".into()),
        "enabled".into(),
    );
    resources.values.borrow_mut().insert(
        (component_name.clone(), "Emacs.MenuBar.Frame".into()),
        "frame-enabled".into(),
    );
    i.set_x_resource_backend(Some(resources.clone()));

    let result = printed(&mut i, r#"(x-get-resource "menuBar" "MenuBar")"#);
    let calls = resources.requests.borrow().clone();
    let inhibit = printed(&mut i, "inhibit-x-resources");
    let window_system = printed(&mut i, "(window-system)");
    assert_eq!(
        result, "\"enabled\"",
        "calls={calls:?}, inhibit={inhibit}, window-system={window_system}"
    );
    assert_eq!(
        resources.requests.borrow().last().unwrap(),
        &(simple_name, "Emacs.MenuBar".into())
    );
    assert_eq!(
        printed(
            &mut i,
            r#"(x-get-resource "menuBar" "MenuBar" "frame" "Frame")"#
        ),
        "\"frame-enabled\""
    );
    assert_eq!(
        resources.requests.borrow().last().unwrap(),
        &(component_name, "Emacs.MenuBar.Frame".into())
    );
    assert!(eval(&mut i, r#"(x-get-resource "missing" "Missing")"#).is_nil());
    i.set_x_resource_backend(None);
    assert!(eval(&mut i, r#"(x-get-resource "menuBar" "MenuBar")"#).is_nil());

    i.set_x_resource_backend(Some(resources.clone()));
    let calls = resources.requests.borrow().len();
    eval(&mut i, "(setq inhibit-x-resources t)");
    assert!(eval(&mut i, r#"(x-get-resource "menuBar" "MenuBar")"#).is_nil());
    assert_eq!(resources.requests.borrow().len(), calls);
    eval(&mut i, "(setq inhibit-x-resources nil)");

    assert!(matches!(
        i.eval_str("(x-get-resource nil \"Class\")"),
        Err(Flow::Signal(_))
    ));
    assert!(matches!(
        i.eval_str("(x-get-resource \"name\" \"Class\" \"component\")"),
        Err(Flow::Signal(_))
    ));
}
