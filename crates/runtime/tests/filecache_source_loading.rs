use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (defvar remacs-2985-file nil)
  (defvar remacs-2985-truename nil)
  (unless (and (fboundp 'file-cache--add-from-nul-buffer)
               (fboundp 'file-cache--directory-member)
               (fboundp 'file-cache-add-directory-using-find)
               (fboundp 'file-cache-clear-cache))
    (error "filecache definitions after NUL-buffer parser were not loaded"))
  (let ((file-cache-alist nil)
        (file-cache-filter-regexps nil))
    (setq remacs-2985-file (make-temp-file "remacs-filecache-2985-"))
    (unwind-protect
        (progn
          (setq remacs-2985-truename (file-truename remacs-2985-file))
          (with-temp-buffer
            (insert remacs-2985-file (char-to-string 0))
            (goto-char (point-min))
            (file-cache--add-from-nul-buffer))
          (unless (member (list (file-name-nondirectory remacs-2985-truename)
                                (file-name-directory remacs-2985-truename))
                          file-cache-alist)
            (error "NUL-delimited file cache entry missing: %S" file-cache-alist)))
      (when (file-exists-p remacs-2985-file)
        (delete-file remacs-2985-file))))
  t)"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn filecache_source_loads_and_imports_nul_delimited_names() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/filecache.el")
        .canonicalize()
        .unwrap();
    let code = format!("(progn (load {} nil t t) {})", lisp_string(source.to_str().unwrap()), CASES);
    let mut interp = Interp::new();
    match interp.eval_str(&code) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
