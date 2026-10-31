use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (defun remacs-test-eww-hidden-submit (method children &optional table filename)
    (with-temp-buffer
      (let* ((eww-data (list :url "https://example.test/page"))
             (shr-use-fonts nil)
             (shr-width 100)
             (shr-external-rendering-functions
              '((form . eww-tag-form) (input . eww-tag-input)
                (button . eww-form-submit) (textarea . eww-tag-textarea)
                (select . eww-tag-select)))
             (form (append (list 'form (list (cons 'method method)
                                            (cons 'action "https://example.test/submit")))
                           (if table
                               (list (list 'table nil
                                           (append (list 'tr nil)
                                                   (mapcar (lambda (child) (list 'td nil child))
                                                           children))))
                             children)))
             observed)
        (shr-insert-document (list 'html nil (list 'body nil form)))
        (goto-char (point-min))
        (let (submit)
          (while (< (point) (point-max))
            (let ((field (get-text-property (point) 'eww-form)))
              (when (equal (plist-get field :type) "submit")
                (setq submit (point)))
              (when (and filename (equal (plist-get field :type) "file"))
                (plist-put field :filename filename)))
            (forward-char))
          (unless submit (error "Rendered form has no submit button"))
          (goto-char submit))
        (cl-letf (((symbol-function 'eww-browse-url)
                   (lambda (url &rest _)
                     (setq observed (list url url-request-method url-request-data
                                          url-request-extra-headers)))))
          (eww-submit))
        observed)))

  (dolist (method '("get" "post"))
    (dolist (table '(nil t))
      (dolist (name '("" nil))
        (let* ((attrs (append '((type . "hidden") (value . "secret"))
                              (when name (list (cons 'name name)))))
               (result (remacs-test-eww-hidden-submit
                        method (list (dom-node 'input attrs) (dom-node 'button nil "Send")) table))
               (query (if (equal method "post") (nth 2 result)
                        (cdr (url-path-and-query (url-generic-parse-url (car result)))))))
          (unless (equal query "")
            (error "Nameless hidden control leaked into %S submission: %S" method result))))
      (let* ((children
              (list (dom-node 'input '((type . "hidden") (name . "") (value . "empty-secret")))
                    (dom-node 'input '((type . "hidden") (value . "missing-secret")))
                    (dom-node 'input '((name . "q") (value . "hello")))
                    (dom-node 'input '((type . "hidden") (name . "keep") (value . "first")))
                    (dom-node 'input '((type . "hidden") (name . "keep") (value . "second")))
                    (dom-node 'input '((type . "hidden") (name . "empty")))
                    (dom-node 'input '((type . "hidden") (name . " ") (value . "space")))
                    (dom-node 'input '((type . "hidden") (name . "0") (value . "zero")))
                    (dom-node 'button nil "Send")))
             (result (remacs-test-eww-hidden-submit method children table))
             (query (if (equal method "post") (nth 2 result)
                      (cdr (url-path-and-query (url-generic-parse-url (car result)))))))
        (unless (equal query "q=hello&keep=first&keep=second&empty=&+=space&0=zero")
          (error "Hidden name filtering changed named controls: %S" result)))))
  (with-temp-buffer
    (let* ((form (list (cons :method "post")
                       (cons :action "https://example.test/submit")
                       (list 'hidden :name nil :value "missing-secret")
                       (list 'hidden :name "" :value "empty-secret")
                       (list 'hidden :name "keep" :value "ok")))
           (input (list :eww-form form :type "submit"))
           (eww-data (list :url "https://example.test/page"))
           body)
      (insert "Submit")
      (put-text-property (point-min) (point-max) 'eww-form input)
      (goto-char (point-min))
      (cl-letf (((symbol-function 'eww-browse-url)
                 (lambda (&rest _) (setq body url-request-data))))
        (eww-submit))
      (unless (equal body "keep=ok")
        (error "Stored form state bypassed hidden name validation: %S" body))))
  (let ((file (make-temp-file "remacs-eww-name-upload-")))
    (unwind-protect
        (progn
          (with-temp-file file (insert "file-payload-2672"))
          (let* ((children
                  (list (dom-node 'input '((type . "hidden") (value . "missing-secret-2672")))
                        (dom-node 'input '((type . "hidden") (name . "")
                                           (value . "empty-secret-2672")))
                        (dom-node 'input '((type . "file") (name . "upload")))
                        (dom-node 'input '((type . "hidden") (name . "keep")
                                           (value . "keep-marker-2672")))
                        (dom-node 'button nil "Send")))
                 (body (nth 2 (remacs-test-eww-hidden-submit "post" children nil file))))
            (unless (and (string-search "file-payload-2672" body)
                         (string-search "keep-marker-2672" body)
                         (not (string-search "missing-secret-2672" body))
                         (not (string-search "empty-secret-2672" body)))
              (error "Multipart submission bypassed hidden name validation: %S" body))))
      (delete-file file)))
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
fn hidden_controls_require_a_nonempty_name() {
    let mut interp = Interp::new();
    eprintln!("default EWW hidden-name validation");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW hidden-name validation");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW hidden-name validation");
    check(&mut interp, CASES);
}
