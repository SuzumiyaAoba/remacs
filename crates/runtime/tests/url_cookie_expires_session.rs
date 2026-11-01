use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const PRELOAD: &str = r##"(progn
  ;; Prevent url-cookie's library load from starting a save timer or using the
  ;; user's cookie file.  url-cookie.el is loaded only after this state exists.
  (defvar url-setup-done nil)
  (setq url-setup-done nil)
  (defvar url-cookie-storage nil)
  (defvar url-cookie-secure-storage nil)
  (defvar url-cookie-file nil)
  (defvar url-cookie-save-interval nil)
  (defvar url-cookie-timer nil)
  (defvar url-cookies-changed-since-last-save nil)
  (defvar url-cookie-confirmation nil)
  (defvar url-cookie-trusted-urls nil)
  (defvar url-cookie-untrusted-urls nil)
  (defvar url-privacy-level nil)
  (setq url-cookie-storage nil
        url-cookie-secure-storage nil
        url-cookie-file nil
        url-cookie-save-interval nil
        url-cookie-timer nil
        url-cookies-changed-since-last-save nil
        url-cookie-confirmation nil
        url-cookie-trusted-urls nil
        url-cookie-untrusted-urls nil
        url-privacy-level nil)
  t)"##;

const CASES: &str = r##"(progn
  (require 'cl-lib)
  ;; Reassert the no-I/O state after the source load.  No file, timer,
  ;; confirmation prompt, trust policy, or network operation is used.
  (setq url-setup-done nil
        url-cookie-storage nil
        url-cookie-secure-storage nil
        url-cookie-file nil
        url-cookie-save-interval nil
        url-cookie-timer nil
        url-cookie-confirmation nil
        url-cookie-trusted-urls nil
        url-cookie-untrusted-urls nil
        url-privacy-level nil)
  (defun remacs-cookie-expires-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-cookie-expires-check (name date invalid)
    (let ((url-cookie-storage nil)
          (url-cookie-secure-storage nil)
          (url-cookie-file nil)
          (url-cookie-save-interval nil)
          (url-cookie-timer nil)
          (url-cookie-confirmation nil)
          (url-cookie-trusted-urls nil)
          (url-cookie-untrusted-urls nil)
          (url-privacy-level nil)
          (url-cookies-changed-since-last-save nil))
      (with-temp-buffer
        (setq-local url-current-object
                    (url-generic-parse-url
                     "https://example.invalid/a/page"))
        (let ((parser-signalled
               (condition-case nil
                   (progn (url-cookie--expires-date-to-time date) nil)
                 (error t))))
          (and (eq parser-signalled invalid)
               (progn
                 ;; Exercise the public Set-Cookie path.  Invalid Expires is
                 ;; ignored as a date attribute, but the cookie remains.
                 (url-cookie-handle-set-cookie
                  (concat name "=retained; Expires=" date "; Path=/a"))
                 (let* ((domain-cookies
                         (cdr (assoc "example.invalid" url-cookie-storage)))
                        (cookie (car domain-cookies)))
                   (and (= (length domain-cookies) 1)
                        (url-cookie-p cookie)
                        (equal name (url-cookie-name cookie))
                        (equal "retained" (url-cookie-value cookie))
                        (equal "/a" (url-cookie-localpart cookie))
                        (if invalid
                            (let ((retrieved
                                   (url-cookie-retrieve
                                    "example.invalid" "/a/next")))
                              (and (null (url-cookie-expires cookie))
                                   (not (url-cookie-expired-p cookie))
                                   (equal retrieved (list cookie))))
                          ;; Check acceptance without relying on the wall clock.
                          (equal date (url-cookie-expires cookie)))))))))))
  (remacs-cookie-expires-assert
   (null url-cookie-timer)
   "loading url-cookie.el unexpectedly started a save timer")
  (dolist (case
           '(("year1600" "Wed, 21 Oct 1600 07:28:00 GMT" t)
             ("missing-time" "Tue, 22 Oct 2030 GMT" t)
             ("missing-day" "Tue, Oct 2030 07:28:00 GMT" t)
             ("invalid-time" "Tue, 22 Oct 2030 25:61:61 GMT" t)
             ("valid" "Tue, 22 Oct 2030 07:28:00 GMT" nil)))
    (remacs-cookie-expires-assert
     (remacs-cookie-expires-check (nth 0 case) (nth 1 case) (nth 2 case))
     (format "Set-Cookie Expires handling failed for %s" (nth 0 case))))
  t)"##;

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

fn run_with_library(relative: &str) {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    eval(&mut interp, PRELOAD);
    let path = root.join(relative);
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
    );
    assert_eq!(eval(&mut interp, CASES), "t");
}

#[test]
fn invalid_expires_keep_session_cookies_from_source() {
    run_with_library("lisp/url-cookie.el");
}

#[test]
fn invalid_expires_keep_session_cookies_from_compiled_load() {
    run_with_library("lisp-elc/url-cookie.elc");
}
