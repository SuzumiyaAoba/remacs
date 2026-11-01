use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (require 'cl-lib)
  (defvar find-program "find")
  (defvar remacs-3072-process-calls nil)
  (defvar remacs-3072-shell-calls nil)
  (defun remacs-3072-check (label condition)
    (unless condition
      (error "ido-wide-find-dirs-or-files #3072 failed: %S" label))
    t)
  (defun remacs-3072-process-stub (program infile destination display &rest args)
    (push (list program infile destination display args)
          remacs-3072-process-calls)
    0)
  (defun remacs-3072-shell-stub (command)
    (push command remacs-3072-shell-calls)
    "")
  (let* ((find-program "/opt/Tools and Apps/find;$(literal)")
         (dir "/tmp/project root;$(literal)")
         (folded-pattern "*name with space;$(token)*")
         (prefix-pattern "sub name;$(token)*")
         (remacs-3072-process-calls nil)
         (remacs-3072-shell-calls nil))
    (cl-letf (((symbol-function 'process-file) #'remacs-3072-process-stub)
              ((symbol-function 'shell-command-to-string) #'remacs-3072-shell-stub))
      (let ((ido-case-fold t))
        (ido-wide-find-dirs-or-files dir "name with space;$(token)" nil t))
      (let ((ido-case-fold nil))
        (ido-wide-find-dirs-or-files dir "sub name;$(token)" t nil)))
    (let ((calls (nreverse remacs-3072-process-calls)))
      (remacs-3072-check 'argv-and-no-shell
       (and
       (equal calls
              (list
               (list find-program nil t nil
                     (list dir "-iname" folded-pattern "-type" "d" "-print"))
               (list find-program nil t nil
                     (list dir "-name" prefix-pattern "-type" "f" "-print"))))
       (null remacs-3072-shell-calls)))))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn ido_wide_find_passes_program_and_patterns_as_separate_arguments() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/ido.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t) {})",
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
    assert!(result.truthy());
}
