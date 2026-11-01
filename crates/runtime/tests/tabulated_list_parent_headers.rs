use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (unless (fboundp 'tabulated-list-groups-flatten)
    (error "tabulated-list-groups-flatten is not defined in this phase"))
  (defvar remacs-3087-parent-entry nil)
  (let* ((sibling-groups
          (tabulated-list-groups
           '(("child-b") ("child-c"))
           (list :path-function
                 (lambda (entry)
                   (if (equal (car entry) "child-b")
                       '(("A" "B"))
                     '(("A" "C")))))))
         (parent-groups
          (progn
            (setq remacs-3087-parent-entry
                  (list (propertize "parent" 'face 'bold)))
            (tabulated-list-groups
             (list remacs-3087-parent-entry '("child-b") '("child-c"))
             (list :path-function
                   (lambda (entry)
                     (cond ((equal (car entry) "parent") '(("A")))
                           ((equal (car entry) "child-b") '(("A" "B")))
                           (t '(("A" "C"))))))))))
    (unless (equal sibling-groups
                   '(("* A\n** B" ("child-b"))
                     ("* A\n** C" ("child-c"))))
      (error "Sibling group lost its common parent: %S" sibling-groups))
    (unless (and (equal parent-groups
                        '(("* A" ("parent"))
                          ("* A\n** B" ("child-b"))
                          ("* A\n** C" ("child-c"))))
                 (eq (cadr (car parent-groups)) remacs-3087-parent-entry)
                 (eq (get-text-property 0 'face
                                        (car remacs-3087-parent-entry))
                     'bold))
      (error "Parent entries/headings were flattened incorrectly: %S"
             parent-groups))
    (unless (equal (tabulated-list-groups-flatten '(("empty" . [])))
                   '(("* empty")))
      (error "An empty leaf group was dropped"))
    (unless (null (tabulated-list-groups-flatten '(("empty" . nil))))
      (error "An empty internal group should remain omitted")))
  (unless (equal (tabulated-list-groups-flatten
                  '(("A" ("empty-child" . []))))
                 '(("* A\n** empty-child")))
    (error "An empty child group lost its hierarchy"))
  (unless (equal (tabulated-list-groups-flatten
                  '(("vector-root" . ["entry"])))
                 '(("* vector-root" "entry")))
    (error "A vector root sequence was not accepted"))
  (unless (equal (tabulated-list-groups-flatten
                  '(("A" . (["parent"] . [("B" . ["child"])]))))
                 '(("* A" "parent") ("* A\n** B" "child")))
    (error "A vector child sequence lost its hierarchy"))
  (let ((type-error
         (condition-case err
             (progn (tabulated-list-groups-flatten '(("bad" . 7))) nil)
           (wrong-type-argument err))))
    (unless (equal type-error '(wrong-type-argument sequencep 7))
      (error "Malformed group tree error changed: %S" type-error)))
  t)"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn check(interp: &mut Interp, phase: &str, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy(), "{phase} returned nil"),
        Err(Flow::Signal(signal)) => panic!(
            "{phase}: {} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{phase}: {other:?}"),
    }
}

#[test]
fn grouped_tabulated_list_preserves_parent_paths_and_entry_objects() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let aot = root
        .join("lisp-elc/tabulated-list.elc")
        .canonicalize()
        .unwrap();
    let source = root
        .join("lisp/tabulated-list.el")
        .canonicalize()
        .unwrap();
    let mut interp = Interp::new();

    // Interp::new boots tabulated-list from the AOT boot-library table.
    check(&mut interp, "boot definition", CASES);
    check(
        &mut interp,
        "direct AOT load",
        &format!("(load {} nil t t)", lisp_string(aot.to_str().unwrap())),
    );
    check(&mut interp, "AOT definition", CASES);
    let source_load = format!("(load {} nil t t)", lisp_string(source.to_str().unwrap()));
    check(&mut interp, "source load", &source_load);
    check(&mut interp, "source definition", CASES);
    check(&mut interp, "source reload", &source_load);
    check(&mut interp, "reloaded source definition", CASES);
}
