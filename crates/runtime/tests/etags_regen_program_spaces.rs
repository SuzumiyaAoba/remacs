use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (require 'cl-lib)
  (defvar remacs-etags-root nil)
  (defvar remacs-etags-tags nil)
  (defvar remacs-etags-calls nil)
  (defun remacs-etags-version (command)
    (push (list 'version command) remacs-etags-calls)
    "Universal Ctags")
  (defun remacs-etags-region (start end program output errors &rest args)
    (push (list (if start 'generate 'append) program output errors args
                (and start (buffer-substring-no-properties start end)))
          remacs-etags-calls))
  (defun remacs-etags-project-root (_project) remacs-etags-root)
  (defun remacs-etags-files (_project)
    (list (expand-file-name "one.el" remacs-etags-root)
          (expand-file-name "two.el" remacs-etags-root)))
  (defun remacs-etags-tags-file (_project) remacs-etags-tags)
  (let* ((remacs-etags-root
          (file-name-as-directory (make-temp-file "etags program spaces " t)))
         (program (expand-file-name "etags ;$(literal) binary" remacs-etags-root))
         (remacs-etags-tags (expand-file-name "TAGS output" remacs-etags-root))
         (default-directory remacs-etags-root)
         (exec-path (list remacs-etags-root))
         (etags-regen-program-options '("--option=some value"))
         (etags-regen-regexp-alist nil))
    (unwind-protect
        (progn
          ;; Only shell/process calls are captured.  The path lookup is real.
          (with-temp-file program (insert ""))
          (set-file-modes program #o700)
          (unless (file-executable-p program)
            (error "Fixture program lacks executable permission"))
          (dolist (case (list (list program (shell-quote-argument program))
                             (list "ctags -e" "ctags -e")
                             (let ((quoted (concat (shell-quote-argument program) " -e")))
                               (list quoted quoted))))
            (let ((etags-regen-program (car case))
                  (prefix (cadr case))
                  (remacs-etags-calls nil)
                  (etags-regen--tags-root remacs-etags-root)
                  (etags-regen--tags-file remacs-etags-tags))
              (cl-letf (((symbol-function 'shell-command-to-string)
                         #'remacs-etags-version)
                        ((symbol-function 'etags-regen--process-file-region)
                         #'remacs-etags-region)
                        ((symbol-function 'project-root)
                         #'remacs-etags-project-root)
                        ((symbol-function 'etags-regen--all-files)
                         #'remacs-etags-files)
                        ((symbol-function 'etags-regen--choose-tags-file)
                         #'remacs-etags-tags-file)
                        ((symbol-function 'etags-regen--visit-table) #'ignore))
                (unless (etags-regen--ctags-p)
                  (error "Ctags detection changed"))
                (etags-regen--tags-generate 'project)
                (with-temp-buffer
                  (setq default-directory remacs-etags-root)
                  (etags-regen--append-tags
                   (expand-file-name "one.el" remacs-etags-root)
                   (expand-file-name "two.el" remacs-etags-root))))
              (let* ((version (list 'version (concat prefix " --version")))
                     (option (shell-quote-argument "--option=some value"))
                     (generate (format "%s %s -L - -o %s" prefix option
                                       (shell-quote-argument remacs-etags-tags)))
                     (append (format "%s %s -o - one.el two.el" prefix option))
                     (expected
                      (list version version
                            (list 'generate shell-file-name nil
                                  etags-regen--errors-buffer-name
                                  (list shell-command-switch generate)
                                  "one.el\ntwo.el\n")
                            version
                            (list 'append shell-file-name t
                                  etags-regen--errors-buffer-name
                                  (list shell-command-switch append) nil))))
                (unless (equal (nreverse remacs-etags-calls) expected)
                  (error "Program %S: expected %S, got %S"
                         etags-regen-program expected remacs-etags-calls))))))
      (delete-directory remacs-etags-root t)))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn etags_regen_quotes_program_paths_and_preserves_command_arguments() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/etags-regen.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
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
