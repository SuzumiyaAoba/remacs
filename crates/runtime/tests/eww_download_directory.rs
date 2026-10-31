use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (let* ((root (make-temp-file "remacs-eww-download-" t))
         (directory (expand-file-name "downloads" root))
         (outside (expand-file-name "outside" root)))
    (unwind-protect
        (progn
          (make-directory directory)
          (with-temp-file outside (insert "outside sentinel"))
          (dolist (case (append
                         '(("foo/../../escape.txt" "escape.txt")
                           ("../escape.txt" "escape.txt")
                           ("foo/" "!") ("" "!") ("." "!.") (".." "!..")
                           (".hidden" "!.hidden") ("plain.txt" "plain.txt"))
                         (list (list (expand-file-name "absolute.txt" root) "absolute.txt"))))
            (let ((actual (eww-make-unique-file-name (car case) directory))
                  (expected (expand-file-name (cadr case) directory)))
              (unless (and (equal actual expected)
                           (equal (file-name-directory actual)
                                  (file-name-as-directory directory)))
                (error "Download filename %S escaped its directory: %S" (car case) actual))))
          (let ((count 0))
            (dolist (case
                     (list
                      (list "/base/foo%2F..%2F..%2Foutside" "outside")
                      (list "/base/foo%2F..%2F..%2Foutside" "outside(1)")
                      (list "/base/child%2f..%2f..%2flower.txt" "lower.txt")
                      (list (concat "/base/"
                                    (url-hexify-string (expand-file-name "absolute.txt" root)))
                            "absolute.txt")
                      (list "/base/foo%2F" "!")
                      (list "/base/literal%252F..%252F..%252Ftail.txt"
                            "literal%2F..%2F..%2Ftail.txt")
                      (list "/base/literal%23name%3Fvalue.txt?ignored=/elsewhere#frag"
                            "literal#name?value.txt")))
              (let ((payload (format "download payload %d" count))
                    (expected (expand-file-name (cadr case) directory)))
                (setq count (1+ count))
                (with-temp-buffer
                  (insert "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n" payload)
                  (eww-download-callback nil (concat "https://example.test" (car case)) directory))
                (with-temp-buffer
                  (insert-file-contents-literally expected)
                  (unless (equal (buffer-string) payload)
                    (error "Downloaded payload was not saved under %S" expected))))))
          (with-temp-buffer
            (insert-file-contents-literally outside)
            (unless (equal (buffer-string) "outside sentinel")
              (error "A download overwrote the outside sentinel")))
          (unless (equal (sort (directory-files root nil "^[^.]") #'string-lessp)
                         '("downloads" "outside"))
            (error "A download created a file outside the download directory")))
      (delete-directory root t)))
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
fn download_filenames_stay_inside_the_download_directory() {
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
    let start = source.find("(defun eww-make-unique-file-name ").unwrap();
    let end = source[start..]
        .find("(defun eww-set-character-encoding ")
        .unwrap()
        + start;
    check(&mut interp, &source[start..end]);
    check(&mut interp, CASES);
}
