use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'eww)
  (dolist (folding '(nil t))
    (let ((case-fold-search folding)
          (eww-search-prefix "https://search.invalid/?q="))
      (dolist (input '("file:/tmp/Case%2FName" "FILE:/tmp/Case%2FName"
                        "FiLe:///tmp/Case%2FName" "FILE://host/Case"))
        (unless (equal (eww--dwim-expand-url (concat " \t" input "\n")) input)
          (error "File scheme was mangled with case-fold-search=%S: %S" folding input)))
      (dolist (input '("ftp://example.org/a" "FTP://example.org/Case"
                        "FtP://example.org/a"))
        (unless (equal (condition-case err
                           (list 'accepted (eww--dwim-expand-url (concat " " input " ")))
                         (user-error err))
                       '(user-error "FTP is not supported"))
          (error "FTP scheme escaped rejection with case-fold-search=%S: %S" folding input)))
      (unless (and (equal (eww--dwim-expand-url "HTTPS://example.org/Case%2FPath")
                           "HTTPS://example.org/Case%2FPath")
                   (equal (eww--dwim-expand-url "ftp.example.org/a")
                           "http://ftp.example.org/a")
                   (eq case-fold-search folding))
        (error "Scheme handling changed an accepted URL or caller search binding"))
      (let ((eww-local-regex "\\`LOCAL\\'"))
        (unless (and (equal (eww--dwim-expand-url "LOCAL") "http://LOCAL/")
                     (equal (eww--dwim-expand-url "local")
                            (if folding "http://local/" "https://search.invalid/?q=local")))
          (error "Scheme handling changed custom local-regexp case sensitivity")))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn scheme_detection_ignores_caller_search_case_folding() {
    let mut interp = Interp::new();
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    check(&mut interp, CASES);

    let source = include_str!("../../../lisp/eww.el");
    let start = source.find("(defun eww--dwim-expand-url ").unwrap();
    let end = source[start..]
        .find("(defun eww--preprocess-html ")
        .unwrap()
        + start;
    check(&mut interp, &source[start..end]);
    check(&mut interp, CASES);
}
