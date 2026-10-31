use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (defvar remacs-shell-3184-directory-calls nil)
  (defun remacs-shell-3184-capture-directory (directory)
    (setq remacs-shell-3184-directory-calls
          (cons directory remacs-shell-3184-directory-calls)))
  (defun remacs-shell-3184-track (command)
    (let ((shell-dirtrack-mode t)
          (remacs-shell-3184-directory-calls nil)
          (original (symbol-function 'shell-process-cd)))
      (unwind-protect
          (progn
            (fset 'shell-process-cd #'remacs-shell-3184-capture-directory)
            (shell-directory-tracker command)
            (nreverse remacs-shell-3184-directory-calls))
        (fset 'shell-process-cd original))))
  (list
   (equal (shell-directory-tracker--cd-directory "cd -P -e /tmp") "/tmp")
   (equal (shell-directory-tracker--cd-directory "cd -Pe /tmp") "/tmp")
   (equal (shell-directory-tracker--cd-directory "cd -eP /tmp") "/tmp")
   (equal (shell-directory-tracker--cd-directory "cd -P -e -- -e") "-e")
   (equal (shell-directory-tracker--cd-directory "cd -- -e") "-e")
   (equal (shell-directory-tracker--cd-directory "cd -- -Pe") "-Pe")
   (equal (shell-directory-tracker--cd-directory "cd -p /tmp") "-p")
   (null (shell-directory-tracker--cd-directory "cd"))
   (equal (shell-directory-tracker--cd-directory "cd \"\"") "")
   (null (remacs-shell-3184-track "cd \"\""))
   (equal (remacs-shell-3184-track "cd") '(""))
   (equal (remacs-shell-3184-track "cd -P -e /tmp") '("/tmp"))))"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn shell_directory_tracker_skips_bash_cd_physical_and_exit_options() {
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
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t t t t t t t t)");
}
