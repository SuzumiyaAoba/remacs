use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (unless (and (fboundp 'cd) (fboundp 'locate-file-completion-table))
    (error "The current boot does not define files.el completion functions"))
  (require 'cl-lib)
  (defvar remacs-b17-cd-candidates nil)
  (defvar remacs-b17-bare-valid nil)
  (defvar remacs-b17-bare-try nil)
  (defvar remacs-b17-actions-preserved nil)
  (defvar remacs-b17-subdir-candidates nil)
  (defvar remacs-b17-subdir-try nil)
  (defvar remacs-b17-subdir-valid nil)
  (defvar remacs-b17-cd-root nil)
  (defvar remacs-b17-cd-choice "a/")
  (defun remacs-b17-read-directory-name (&rest _arguments)
    ;; Run the setup hook registered by `cd' without opening a minibuffer.
    (run-hooks 'minibuffer-setup-hook)
    (setq remacs-b17-cd-candidates
          (all-completions "a" minibuffer-completion-table
                           minibuffer-completion-predicate))
    (setq remacs-b17-bare-valid
          (test-completion "a" minibuffer-completion-table
                           minibuffer-completion-predicate))
    (setq remacs-b17-bare-try
          (try-completion "a" minibuffer-completion-table
                          minibuffer-completion-predicate))
    (setq remacs-b17-subdir-candidates
          (all-completions "sub/a" minibuffer-completion-table
                           minibuffer-completion-predicate))
    (setq remacs-b17-subdir-try
          (try-completion "sub/a" minibuffer-completion-table
                          minibuffer-completion-predicate))
    (setq remacs-b17-subdir-valid
          (test-completion "sub/a" minibuffer-completion-table
                           minibuffer-completion-predicate))
    (let ((generic (apply-partially #'locate-file-completion-table cd-path nil)))
      (setq remacs-b17-actions-preserved
            (and
             (equal (funcall minibuffer-completion-table "a"
                             minibuffer-completion-predicate '(boundaries . ""))
                    (funcall generic "a" minibuffer-completion-predicate
                             '(boundaries . "")))
             (equal (funcall minibuffer-completion-table "a"
                             minibuffer-completion-predicate 'metadata)
                    (funcall generic "a" minibuffer-completion-predicate
                             'metadata))
             ;; Absolute names bypass CDPATH filtering for every action.
             (equal (funcall minibuffer-completion-table remacs-b17-cd-root
                             minibuffer-completion-predicate t)
                    (funcall generic remacs-b17-cd-root
                             minibuffer-completion-predicate t))
             (equal (try-completion remacs-b17-cd-root minibuffer-completion-table
                                    minibuffer-completion-predicate)
                    (try-completion remacs-b17-cd-root generic
                                    minibuffer-completion-predicate))
             (equal (test-completion remacs-b17-cd-root minibuffer-completion-table
                                  minibuffer-completion-predicate)
                 (test-completion remacs-b17-cd-root generic
                                  minibuffer-completion-predicate))))
    remacs-b17-cd-choice))
  (let* ((root (file-name-as-directory
                (make-temp-file "remacs-cdpath-1671-" t)))
         (dir-a (file-name-as-directory (expand-file-name "A" root)))
         (dir-b (file-name-as-directory (expand-file-name "B" root)))
         (dir-a-a (file-name-as-directory (expand-file-name "a" dir-a)))
         (dir-b-a (file-name-as-directory (expand-file-name "a" dir-b)))
         (dir-a-sub (file-name-as-directory (expand-file-name "sub" dir-a)))
         (dir-b-sub (file-name-as-directory (expand-file-name "sub" dir-b)))
         (dir-b-sub-a (file-name-as-directory (expand-file-name "a" dir-b-sub))))
    (unwind-protect
        (progn
          (setq remacs-b17-cd-root root)
          (make-directory dir-a)
          (make-directory dir-b)
          (with-temp-file (expand-file-name "a" dir-a)
            (insert "file"))
          (make-directory (expand-file-name "a" dir-b))
          (make-directory dir-a-sub)
          (make-directory dir-b-sub)
          (with-temp-file (expand-file-name "a" dir-a-sub)
            (insert "file"))
          (make-directory (expand-file-name "a" dir-b-sub))
          (with-temp-buffer
            (setq default-directory root)
            (let* ((cd-path (list dir-a dir-b))
                   (table (apply-partially #'locate-file-completion-table
                                           cd-path nil))
                   (generic-candidates (all-completions "a" table nil)))
              ;; Ordinary generic file completion still offers both actual
              ;; entries: a regular file `a' and a directory `a/'.
              (unless (and (= (length generic-candidates) 2)
                           (member "a" generic-candidates)
                           (member "a/" generic-candidates))
                (error "Generic file/dir candidates changed: %S"
                       generic-candidates))
              ;; `cd' must filter the file-derived bare name when its
              ;; directory predicate resolves only through a later CDPATH item.
              (let ((minibuffer-setup-hook nil))
                (cl-letf (((symbol-function 'read-directory-name)
                           #'remacs-b17-read-directory-name))
                  (call-interactively 'cd)))
              (unless (equal remacs-b17-cd-candidates '("a/"))
                (error "Interactive cd completions were %S"
                       remacs-b17-cd-candidates))
              (unless (equal default-directory dir-b-a)
                (error "Interactive cd resolved to %S, expected %S"
                       default-directory dir-b-a))
              (unless (and remacs-b17-bare-valid
                           (equal remacs-b17-bare-try "a/")
                           (equal remacs-b17-subdir-candidates '("a/"))
                           (equal remacs-b17-subdir-try "sub/a/")
                           remacs-b17-subdir-valid
                           remacs-b17-actions-preserved)
                (error "CDPATH action/prefix behavior changed: %S / %S / %S"
                       remacs-b17-bare-try remacs-b17-subdir-candidates
                       remacs-b17-subdir-try))
              ;; The hidden bare name is still valid when entered manually;
              ;; the completion display must not make it unenterable.
              (setq remacs-b17-cd-choice "a"
                    default-directory root)
              (let ((minibuffer-setup-hook nil))
                (cl-letf (((symbol-function 'read-directory-name)
                           #'remacs-b17-read-directory-name))
                  (call-interactively 'cd)))
              (unless (and (equal remacs-b17-cd-candidates '("a/"))
                           (equal default-directory dir-b-a))
                (error "Bare CDPATH input did not resolve to %S: %S / %S"
                       dir-b-a remacs-b17-cd-candidates default-directory))
              (setq remacs-b17-cd-choice "sub/a"
                    default-directory root)
              (let ((minibuffer-setup-hook nil))
                (cl-letf (((symbol-function 'read-directory-name)
                           #'remacs-b17-read-directory-name))
                  (call-interactively 'cd)))
              (unless (equal default-directory dir-b-sub-a)
                (error "Relative subdirectory did not resolve through CDPATH: %S"
                       default-directory))
              ;; Directory display remains canonical regardless of search
              ;; order; the bare form is still accepted as typed input.
              (delete-file (expand-file-name "a" dir-a))
              (delete-directory dir-b-a)
              (make-directory (expand-file-name "a" dir-a))
              (with-temp-file (expand-file-name "a" dir-b)
                (insert "file"))
              (setq remacs-b17-cd-choice "a"
                    default-directory root)
              (let ((minibuffer-setup-hook nil))
                (cl-letf (((symbol-function 'read-directory-name)
                           #'remacs-b17-read-directory-name))
                  (call-interactively 'cd)))
              (unless (and (equal remacs-b17-cd-candidates '("a/"))
                           remacs-b17-bare-valid
                           (equal remacs-b17-bare-try "a/")
                           (equal default-directory dir-a-a))
                (error "Reverse CDPATH order changed canonical directory completion: %S / %S"
                       remacs-b17-cd-candidates default-directory))))
          t)
      (when (file-directory-p root)
        (delete-directory root t)))
  t))"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

fn run_cdpath_case(load_files_source: bool) {
    let mut interp = Interp::new();
    let code = if load_files_source {
        let files = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/files.el");
        format!(
            "(progn (setq noninteractive t) (load {} nil t t) {})",
            lisp_string(files.to_str().unwrap()),
            CASES
        )
    } else {
        format!("(progn (setq noninteractive t) {})", CASES)
    };
    assert_eq!(eval(&mut interp, &code), "t");
    assert_eq!(
        eval(&mut interp, "(let ((file (symbol-file 'files--cd-completion-table 'defun))) (and (stringp file) (file-name-absolute-p file) (file-readable-p file) t))"),
        "t"
    );
}

#[test]
fn cdpath_completion_in_initial_boot_filters_file_derived_directory() {
    run_cdpath_case(false);
}

#[test]
fn cdpath_completion_after_files_source_load_filters_file_derived_directory() {
    run_cdpath_case(true);
}
