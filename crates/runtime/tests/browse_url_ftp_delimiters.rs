use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('\"', "\\\""))
}

const CASES: &str = r#"(progn
  (unless (and (fboundp 'browse-url-file-url)
               (fboundp 'url-generic-parse-url)
               (fboundp 'url-path-and-query)
               (fboundp 'url-unhex-string))
    (error "Browse URL/URL parser source definitions were not loaded"))
  ;; These are only URL strings; no ange-ftp handler or connection is used.
  (dolist (case
           '(("/anonymous@ftp.example.invalid:/dir/a#b?c%"
              "ftp://ftp.example.invalid/dir/a%23b%3Fc%25"
              nil "ftp.example.invalid" "/dir/a#b?c%")
             ("/alice@ftp.example.invalid:/dir/a#b?c%20 space-日本"
              "ftp://alice@ftp.example.invalid/dir/a%23b%3Fc%2520%20space-%E6%97%A5%E6%9C%AC"
              "alice" "ftp.example.invalid" "/dir/a#b?c%20 space-日本")))
    (let* ((input (nth 0 case))
           (expected-url (nth 1 case))
           (expected-user (nth 2 case))
           (expected-host (nth 3 case))
           (expected-path (nth 4 case))
           (url (browse-url-file-url input))
           (parsed (url-generic-parse-url url))
           (path-query (url-path-and-query parsed))
           (decoded-path (decode-coding-string
                          (url-unhex-string (car path-query)) 'utf-8)))
      (unless (and (equal url expected-url)
                   (equal (url-type parsed) "ftp")
                   (equal (url-user parsed) expected-user)
                   (equal (url-host parsed) expected-host)
                   (equal (car path-query)
                          (substring expected-url
                                     (string-match "/dir/" expected-url)))
                   (null (cdr path-query))
                   (null (url-target parsed))
                   (equal decoded-path expected-path))
        (error "FTP file URL changed authority/path: %S => %S (%S)"
               input url parsed))))
  ;; Keep the already-fixed local file URL behavior as a comparison.
  (let* ((input "/tmp/local#part?mode=%20 space-日本")
         (expected-url
          "file:///tmp/local%23part%3Fmode=%2520%20space-%E6%97%A5%E6%9C%AC")
         (url (browse-url-file-url input))
         (parsed (url-generic-parse-url url))
         (path-query (url-path-and-query parsed))
         (decoded-path (decode-coding-string
                        (url-unhex-string (car path-query)) 'utf-8)))
    (unless (and (equal url expected-url)
                 (equal (url-type parsed) "file")
                 (equal (car path-query)
                        "/tmp/local%23part%3Fmode=%2520%20space-%E6%97%A5%E6%9C%AC")
                 (null (cdr path-query))
                 (null (url-target parsed))
                 (equal decoded-path input))
      (error "Local file URL changed: %S => %S (%S)" input url parsed)))
  t)"#;

#[test]
fn browse_url_ftp_path_escapes_filename_delimiters_3180() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let source = root.join("lisp/browse-url.el").canonicalize().unwrap();
    let code = format!(
        "(progn (require 'url-parse) (require 'url-util) (load {} nil t t) {})",
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
    assert!(result.truthy());
}
