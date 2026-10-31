use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'filesets)
  (dolist (name '("a[bc]" "c++" "a.b" "end\\" "literal*?" "name^$"
                  "paren\\(x\\)" "space name" "日本" "[" "]" "(literal)" "x{2}"))
    (let* ((command (propertize name 'face 'bold))
           (output (concat "/usr/local/bin/" name "\n"))
           (calls 0))
      (cl-letf (((symbol-function 'filesets-which-command)
                 (lambda (argument)
                   (unless (eq argument command) (error "Changed the lookup argument"))
                   (setq calls (1+ calls)) output)))
        (unless (and (eq (filesets-which-command-p command) command) (= calls 1)
                     (eq (get-text-property 0 'face command) 'bold))
          (error "Literal command name was not found: %S" name)))))
  (dolist (case '(("a.bc" "/usr/bin/axbc\n")
                  ("a[bc]" "/usr/bin/ab\n")
                  ("[bracket]" "/usr/bin/b\n")
                  ("missing" "")
                  ("a[bc]" "which: no a[bc] in (/bin:/usr/bin)\n")))
    (cl-letf (((symbol-function 'filesets-which-command) (lambda (_) (cadr case))))
      (when (filesets-which-command-p (car case))
        (error "Wrong command matched %S: %S" (car case) (cadr case)))))
  (cl-letf (((symbol-function 'filesets-which-command) (lambda (_) (error "Lookup failed"))))
    (unless (equal (condition-case err (filesets-which-command-p "command") (error err))
                   '(error "Lookup failed"))
      (error "Filesets swallowed the lookup failure")))
  t)
"#;

#[test]
fn filesets_detects_command_names_as_literal_strings() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
