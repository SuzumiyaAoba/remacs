use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  (unless (and (fboundp 'dired--shell-command-final-operator)
               (fboundp 'dired-shell-stuff-it)
               (fboundp 'dired-do-shell-command))
    (error "Dired auxiliary source definitions were not loaded"))
  (defvar remacs-test-dired-3064-captured nil)
  (defun remacs-test-dired-3064-capture (command)
    (push command remacs-test-dired-3064-captured)
    nil)
  (defun remacs-test-dired-3064-run (command files &optional asynchronous)
    (let ((remacs-test-dired-3064-captured nil)
          (dired-confirm-shell-command nil)
          (dired-click-to-select-mode nil)
          (default-directory "/tmp/"))
      ;; Capture at Dired's documented shell-command boundary.  No process is
      ;; started, and the supplied file names are not accessed.
      (cl-letf (((symbol-function 'dired-run-shell-command)
                 #'remacs-test-dired-3064-capture))
        (if asynchronous
            (dired-do-async-shell-command command nil files)
          (dired-do-shell-command command nil files)))
      (nreverse remacs-test-dired-3064-captured)))
  (defun remacs-test-dired-3064-expected-background ()
    (cond ((and (fboundp 'w32-shell-dos-semantics)
                (w32-shell-dos-semantics))
           "start /b echo foo file& ")
          ((eq system-type 'ms-dos) "echo foo file& ")
          (t "echo foo file & wait& ")))
  ;; The scanner tracks operators only in unquoted, unescaped shell syntax.
  ;; A preceding pipe is compound only for non-Windows shell semantics.
  (dolist (case
           '(("echo foo |&" nil nil)
             ("echo foo |&" t ?&)
             ("echo foo \\|&" nil ?&)
             ("echo foo '|'&" nil ?&)
             ("echo foo \"|\"&" nil ?&)
             ("echo foo | &" nil ?&)
             ("echo foo |;" nil ?\;)
             ("echo foo &&" nil nil)
             ("echo foo ||" nil nil)
             ("echo foo ;&" nil nil)
             ("printf '&'" nil nil)
             ("printf \"&\"" nil nil)
             ("printf '&' &" nil ?&)
             ("echo foo \\&" nil nil)
             ("echo foo \\& &" nil ?&)
             ("echo foo # comment &" nil nil)
             ("echo foo ^&" t nil)
             ("echo foo ^&" nil ?&)
             ("echo foo &   " nil ?&)
             ("echo foo & \t" nil ?&)
             ("echo foo ;   " nil ?\;)
             ("echo foo &\n" nil nil)
             ("echo foo\n&" nil ?&)))
    (let* ((command (nth 0 case))
           (windows (nth 1 case))
           (expected (nth 2 case))
           (actual (dired--shell-command-final-operator command windows)))
      (unless (eq (car-safe actual) expected)
        (error "Wrong final operator for %S (windows %S): %S"
               command windows actual))))
  ;; Separators retain their original trimming behavior, including spaces
  ;; before an operator and commands where a newline is final.
  (let* ((command "echo foo ;   ")
         (operator (dired--shell-command-final-operator command)))
    (unless (and (eq (car-safe operator) ?\;)
                 (equal (substring command (cdr operator)) " ;   "))
      (error "Trailing semicolon spacing changed: %S" operator)))
  (unless (null (dired--shell-command-final-operator "echo foo &\n"))
    (error "A newline after the command changed final-operator behavior"))
  ;; Public synchronous Dired preserves Bash's pipe-plus-stderr operator.
  (let* ((windows (and (fboundp 'w32-shell-dos-semantics)
                       (w32-shell-dos-semantics)))
         (expected (if windows
                       "start /b echo foo | file& "
                     "echo foo |& file"))
         (actual (remacs-test-dired-3064-run
                  "echo foo |&" '("file"))))
    (unless (equal actual (list expected))
      (error "Dired rewrote terminal |&: %S" actual)))
  (unless (equal (remacs-test-dired-3064-run "printf '&'" '("file"))
                 '("printf '&' file"))
    (error "Dired treated a quoted ampersand as an operator"))
  (unless (equal (remacs-test-dired-3064-run "printf \\&" '("file"))
                 '("printf \\& file"))
    (error "Dired treated an escaped ampersand as an operator"))
  ;; A real standalone ampersand still takes Dired's background/wait path.
  (let ((expected (remacs-test-dired-3064-expected-background))
        (actual (remacs-test-dired-3064-run "echo foo &" '("file"))))
    (unless (equal actual (list expected))
      (error "Dired background helper output changed: %S" actual)))
  ;; The public async entry point still adds a standalone background suffix.
  (let* ((windows (and (fboundp 'w32-shell-dos-semantics)
                       (w32-shell-dos-semantics)))
         (expected (if windows
                       "start /b echo foo | file& "
                     "echo foo |& file & wait& "))
         (actual (remacs-test-dired-3064-run "echo foo |&" '("file") t)))
    (unless (equal actual (list expected))
      (error "Dired async command changed the compound pipeline operator: %S" actual)))
  (let ((expected (remacs-test-dired-3064-expected-background))
        (actual (remacs-test-dired-3064-run "echo foo" '("file") t)))
    (unless (equal actual (list expected))
      (error "Dired async command output changed: %S" actual)))
  ;; Sequential semicolon handling still batches the command without starting
  ;; a shell process; the runner stub captures the final command string.
  (let* ((windows (and (fboundp 'w32-shell-dos-semantics)
                       (w32-shell-dos-semantics)))
         (expected (if windows
                       "start /b echo foo one& start /b echo foo two"
                     "echo foo one; echo foo two"))
         (actual (remacs-test-dired-3064-run "echo foo ;" '("one" "two"))))
    (unless (equal actual (list expected))
      (error "Dired sequential command output changed: %S" actual)))
  (let* ((windows (and (fboundp 'w32-shell-dos-semantics)
                       (w32-shell-dos-semantics)))
         (expected (if windows
                       "start /b echo foo one& start /b echo foo two& "
                     "echo foo one; echo foo two& "))
         (actual (remacs-test-dired-3064-run "echo foo ;&" '("one" "two") t)))
    (unless (equal actual (list expected))
      (error "Dired sequential background suffix was not preserved: %S" actual)))
  (let* ((windows (and (fboundp 'w32-shell-dos-semantics)
                       (w32-shell-dos-semantics)))
         (expected (cond (windows "start /b printf \\ file& ")
                         ((eq system-type 'ms-dos) "printf \\& file& ")
                         (t "printf \\& file & wait& ")))
         (actual (remacs-test-dired-3064-run "printf \\&" '("file") t)))
    (unless (equal actual (list expected))
      (error "Dired async escaped ampersand output changed: %S" actual)))
  t)"#;

#[test]
fn dired_pipe_stderr_is_not_backgrounded_3064() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let source = root.join("lisp/dired-aux.el").canonicalize().unwrap();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => {
            let symbol = format!("{}", interp.prin1_to_string(&signal.symbol()));
            let data = format!("{}", interp.prin1_to_string(&signal.data()));
            panic!("{symbol} {data}")
        }
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "t");
}
