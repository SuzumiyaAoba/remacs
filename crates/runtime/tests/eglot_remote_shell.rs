use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

#[test]
fn eglot_uses_remote_shell_switch_and_skips_posix_stty_for_other_shells() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/eglot.el");
            let source = std::fs::read_to_string(source).unwrap();
            let start = source.find("(defun eglot--cmd").unwrap();
            let end = start
                + source[start..]
                    .find("(defvar-local eglot--cached-server")
                    .unwrap();
            let definition = &source[start..end];
            let expression = format!(
                r#"(progn
                  (setq hack-read-symbol-shorthands-function nil)
                  (defun file-remote-p (&rest _arguments) t)
                  {definition}
                  (let ((default-directory "/remote/project/")
                        (shell-file-name "powershell.exe")
                        (shell-command-switch "-Command"))
                    (let ((powershell (eglot--cmd '("server" "--stdio")))
                          (posix (let ((shell-file-name "/bin/sh")
                                       (shell-command-switch "-c"))
                                   (eglot--cmd '("server" "--stdio")))))
                      (list (car powershell)
                            (cadr powershell)
                            (string-prefix-p "stty raw > /dev/null;" (nth 2 posix))
                            (string-prefix-p "stty raw > /dev/null;" (nth 2 powershell))
                            (and (string-match-p "server.*--stdio" (nth 2 powershell)) t)))))"#
            );

            let mut interp = Interp::new();
            let value = match interp.eval_str(&expression) {
                Ok(value) => value,
                Err(Flow::Signal(signal)) => panic!(
                    "eglot--cmd failed: {} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                Err(flow) => panic!("eglot--cmd failed: {flow:?}"),
            };
            assert_eq!(
                interp.prin1_to_string(&value).to_unicode_lossy(),
                "(\"powershell.exe\" \"-Command\" t nil t)"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
