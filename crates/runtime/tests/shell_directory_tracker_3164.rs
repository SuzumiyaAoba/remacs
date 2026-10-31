use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (defvar remacs-shell-3164-directory-calls nil)
  (defun remacs-shell-3164-capture-directory (directory)
    (setq remacs-shell-3164-directory-calls
          (cons directory remacs-shell-3164-directory-calls)))
  (defun remacs-shell-3164-track (command)
    (let ((shell-dirtrack-mode t)
          (remacs-shell-3164-directory-calls nil)
          (original (symbol-function 'shell-process-cd)))
      (unwind-protect
          (progn
            (fset 'shell-process-cd #'remacs-shell-3164-capture-directory)
            (shell-directory-tracker command)
            (nreverse remacs-shell-3164-directory-calls))
        (fset 'shell-process-cd original))))
  (list
   (null (remacs-shell-3164-track "echo $(echo x; cd /tmp; echo)"))
   (null (remacs-shell-3164-track "echo \"x; cd /tmp\""))
   (null (remacs-shell-3164-track "echo \"$(echo x; cd /tmp)\""))
   (equal (remacs-shell-3164-track "echo $(echo x; cd /tmp; echo); cd /var/tmp")
          '("/var/tmp"))
   (equal (remacs-shell-3164-track "(cd /tmp; echo); cd /var/tmp")
          '("/var/tmp"))
   (equal (remacs-shell-3164-track "echo x; cd /tmp") '("/tmp"))
   (equal (remacs-shell-3164-track "cd \"/tmp/a;b\"") '("/tmp/a;b"))
   (null (remacs-shell-3164-track "echo x\\; cd /tmp"))
   (null (remacs-shell-3164-track "echo 'x; cd /tmp'"))
   (null (remacs-shell-3164-track "echo `echo x; cd /tmp`"))
   (equal (remacs-shell-3164-track "echo $(echo $(cd /tmp)); cd /var/tmp")
          '("/var/tmp"))
   (equal (remacs-shell-3164-track "echo \"x\ncd /tmp\"; cd /var/tmp")
          '("/var/tmp"))
   (null (remacs-shell-3164-track "echo x | cd /tmp"))
   (null (remacs-shell-3164-track "false && cd /tmp"))))"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn shell_directory_tracker_ignores_nested_and_quoted_cd_commands() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/shell.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (require 'cl-lib) (load {} nil t t) (cl-letf (((symbol-function 'process-command) (lambda (&rest _) '(\"/bin/sh\")))) {}))",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(
        interp.prin1_to_string(&result),
        "(t t t t t t t t t t t t t t)"
    );
}
