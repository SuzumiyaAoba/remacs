use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'eldoc)
  (require 'cl-lib)
  (defvar show-paren-context-when-offscreen nil)
  (defvar show-paren--overlay nil)
  (let ((show-paren-context-when-offscreen t)
        (executing-kbd-macro nil))
    (dolist (show-paren--overlay '(nil 42))
      (unless (eldoc-display-message-no-interference-p)
        (error "A missing show-paren overlay should not block ElDoc")))
    (let ((show-paren--overlay (make-overlay (point-min) (point-min))))
      (unwind-protect
          (progn
            (cl-letf (((symbol-function 'pos-visible-in-window-p)
                       (lambda (&rest _) nil)))
              (when (eldoc-display-message-no-interference-p)
                (error "An offscreen show-paren match should block ElDoc")))
            (cl-letf (((symbol-function 'pos-visible-in-window-p)
                       (lambda (&rest _) t)))
              (unless (eldoc-display-message-no-interference-p)
                (error "A visible show-paren match should allow ElDoc"))))
        (delete-overlay show-paren--overlay)))
    (let ((executing-kbd-macro t)
          (show-paren--overlay nil))
      (when (eldoc-display-message-no-interference-p)
        (error "Keyboard macro interference must still block ElDoc"))))
  t)
"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
}

fn load_source_eldoc(interp: &mut Interp) {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/eldoc.el");
    eval(interp, &format!("(load {} nil t)", lisp_string(source.to_str().unwrap())));
}

#[test]
fn eldoc_guard_tolerates_missing_show_paren_overlay() {
    let mut interp = Interp::new();
    eval(&mut interp, CASES);
    load_source_eldoc(&mut interp);
    eval(&mut interp, CASES);
}
