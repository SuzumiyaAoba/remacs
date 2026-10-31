use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (let ((directory (make-temp-file "remacs-eww-path-" t))
        (url-allow-non-local-files nil))
    (unwind-protect
        (let ((default-directory (file-name-as-directory directory)))
          (dolist (case '(("page#part.html" "page%23part.html")
                          ("page?mode=raw.html" "page%3Fmode%3Draw.html")
                          ("page%23part.html" "page%2523part.html")
                          ("page%2Fpart.html" "page%252Fpart.html")
                          ("page with space.html" "page%20with%20space.html")
                          ("日本語.html" "%E6%97%A5%E6%9C%AC%E8%AA%9E.html")
                          ("name:mail@host;plus+.html" "name:mail%40host%3Bplus%2B.html")
                          ("ordinary.html" "ordinary.html")))
            (let* ((input (car case))
                   (expected-path (concat default-directory (cadr case)))
                   (new-buffer (list 'new-buffer))
                   opened passed-new-buffer passed-allow)
              (with-temp-file input (insert "literal filename payload"))
              (cl-letf (((symbol-function 'eww)
                         (lambda (url &optional arg)
                           (setq opened url passed-new-buffer arg
                                 passed-allow url-allow-non-local-files))))
                (eww-open-file input new-buffer))
              (let* ((parsed (url-generic-parse-url opened))
                     (path-query (url-path-and-query parsed)))
                (unless (and (equal opened (concat "file://" expected-path))
                             (equal (car path-query) expected-path)
                             (null (cdr path-query)) (null (url-target parsed))
                             (eq passed-new-buffer new-buffer) passed-allow
                             (null url-allow-non-local-files))
                  (error "Local filename %S was not encoded as path data: %S" input opened))
                (with-temp-buffer
                  (insert-file-contents-literally (url-file-build-filename parsed))
                  (unless (equal (buffer-string) "literal filename payload")
                    (error "File URL did not read the original filename: %S" input)))))))
      (delete-directory directory t)))
  ;; Check encoded control characters before the file decoder's newline policy.
  (let (opened)
    (cl-letf (((symbol-function 'eww)
               (lambda (url &rest _) (setq opened url))))
      (eww-open-file "/tmp/tabs\tand\nlines.html"))
    (unless (equal opened "file:///tmp/tabs%09and%0Alines.html")
      (error "File URL retained raw whitespace: %S" opened)))
  (dolist (platform '(windows-nt ms-dos))
    (let ((system-type platform) opened passed-new-buffer)
      (cl-letf (((symbol-function 'expand-file-name)
                 (lambda (file &rest _)
                   (unless (equal file "a#b?.html") (error "Changed filename argument"))
                   "C:/Folder/a#b?.html"))
                ((symbol-function 'eww)
                 (lambda (url &optional arg)
                   (setq opened url passed-new-buffer arg))))
        (eww-open-file "a#b?.html" t))
      (unless (and (equal opened "file:///C:/Folder/a%23b%3F.html") passed-new-buffer)
        (error "File URL lost its drive colon, leading slash or new-buffer flag: %S" opened))))
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
fn local_filenames_are_encoded_before_building_file_urls() {
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
    let start = source.find("(defun eww-open-file ").unwrap();
    let end = source[start..]
        .find("(defun eww--file-buffer ")
        .unwrap()
        + start;
    check(&mut interp, &source[start..end]);
    check(&mut interp, CASES);
}
