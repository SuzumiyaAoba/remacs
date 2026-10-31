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
      (dolist (second '("second" "first" ""))
        (let* ((children
                (list (dom-node 'input '((type . "hidden") (name . "tag") (value . "first")))
                      (dom-node 'input (list '(type . "hidden") '(name . "tag")
                                             (cons 'value second)))
                      (dom-node 'button nil "Send")))
               (result (remacs-test-eww-hidden-submit method children table))
               (expected (concat "tag=first&tag=" second))
               (query (if (equal method "post")
                          (nth 2 result)
                        (cdr (url-path-and-query (url-generic-parse-url (car result)))))))
          (unless (equal query expected)
            (error "Hidden controls were lost or reordered (%S table=%S): %S"
                   method table result)))))
    (dolist (table '(nil t))
      (let* ((children
              (list (dom-node 'input '((type . "text") (name . "q") (value . "input")))
                    (dom-node 'input '((type . "hidden") (name . "tag") (value . "first")))
                    (dom-node 'select '((name . "choice"))
                              (dom-node 'option '((value . "A")) "A"))
                    (dom-node 'input '((type . "hidden") (name . "tag") (value . "second")))
                    (dom-node 'input '((type . "checkbox") (checked . "checked")
                                       (name . "tag") (value . "cb")))
                    (dom-node 'textarea '((name . "tag") (rows . "1") (cols . "8")) "notes")
                    (dom-node 'button '((name . "go") (value . "send")) "Send")))
             (result (remacs-test-eww-hidden-submit method children table))
             (query (if (equal method "post") (nth 2 result)
                      (cdr (url-path-and-query (url-generic-parse-url (car result)))))))
        (unless (equal query "q=input&tag=first&choice=A&tag=second&tag=cb&tag=notes&go=send")
          (error "Mixed controls were not sent in document order (%S table=%S): %S"
                 method table result)))))
  (let* ((first (dom-node 'input '((type . "hidden") (name . "tag") (value . "first"))))
         (second (copy-tree first))
         (eww-form (list (cons :method "post"))))
    (with-temp-buffer
      (eww-tag-input first)
      (eww-tag-input first)
      (eww-tag-input second)
      (eww-tag-input second))
    (unless (= (length (cl-remove-if-not (lambda (entry) (eq (car entry) 'hidden)) eww-form)) 2)
      (error "Repeated rendering confused identical controls with the same DOM control")))
  (let ((file (make-temp-file "remacs-eww-upload-")))
    (unwind-protect
        (progn
          (with-temp-file file (insert "file-payload-2671"))
          (let* ((children
                  (list (dom-node 'input '((type . "hidden") (name . "tag")
                                           (value . "first-hidden-2671")))
                        (dom-node 'input '((type . "file") (name . "upload")))
                        (dom-node 'input '((type . "hidden") (name . "tag")
                                           (value . "second-hidden-2671")))
                        (dom-node 'button nil "Send")))
                 (result (remacs-test-eww-hidden-submit "post" children nil file))
                 (body (nth 2 result))
                 (first (string-search "first-hidden-2671" body))
                 (upload (string-search "file-payload-2671" body))
                 (second (string-search "second-hidden-2671" body)))
            (unless (and first upload second (< first upload second)
                         (equal (nth 1 result) "POST")
                         (string-prefix-p "multipart/form-data;"
                                          (cdr (assoc "Content-Type" (nth 3 result)))))
              (error "Multipart controls were lost or reordered: %S" result))))
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
fn hidden_controls_keep_identity_and_document_order() {
    let mut interp = Interp::new();
    eprintln!("default EWW rendering and submission");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW rendering and submission");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW rendering and submission");
    check(&mut interp, CASES);
}
