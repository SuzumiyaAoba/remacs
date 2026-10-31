use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (defvar remacs-test-eww-override-phase nil)
  (defun remacs-test-eww-override-field (id)
    (save-excursion
      (goto-char (point-min))
      (let (found)
        (while (and (< (point) (point-max)) (not found))
          (let ((field (get-text-property (point) 'eww-form)))
            (when (equal (dom-attr (plist-get field :dom) 'id) id)
              (setq found (cons (point) field))))
          (unless found (forward-char)))
        (or found (error "Missing rendered field %S" id)))))
  (defun remacs-test-eww-overrides (attrs buttons trigger expected &optional table extra setup)
    (with-temp-buffer
      (eww-mode)
      (let* ((base "https://example.test/base/page?old=1#old")
             (eww-data (list :url base :title "Overrides"))
             (shr-use-fonts nil) (shr-width 100)
             (nodes (append (list (dom-node 'input '((id . "q") (name . "q")
                                                      (value . "hello world"))))
                            buttons extra))
             (form (append (list 'form attrs)
                           (if table
                               (list (dom-node 'table nil
                                               (append (list 'tr nil)
                                                       (mapcar (lambda (node) (dom-node 'td nil node))
                                                               nodes))))
                             nodes)))
             (url-request-method nil) (url-request-data nil) (url-request-extra-headers nil)
             (attrs-before (copy-tree attrs))
             results)
        (setq remacs-test-eww-override-phase (list attrs trigger table))
        (eww-display-document
         (eww-document-base base (dom-node 'html nil (dom-node 'body nil form))))
        (when setup (funcall setup))
        (dolist (id (if (listp trigger) trigger (list trigger)))
          (goto-char (car (remacs-test-eww-override-field id)))
          (cl-letf (((symbol-function 'eww-browse-url)
                     (lambda (url &rest _)
                       (push (list url url-request-method url-request-data
                                   url-request-extra-headers) results))))
            (eww-submit))
          (unless (and (null url-request-method) (null url-request-data)
                       (null url-request-extra-headers) (eq (plist-get eww-data :url) base)
                       (equal attrs attrs-before))
            (error "Submit overrides leaked into caller/form state")))
        (setq results (nreverse results))
        (let ((actual (if (listp trigger) results (car results))))
          (unless (if (functionp expected) (funcall expected actual) (equal actual expected))
            (error "Submit override mismatch: %S expected %S" actual expected))))))
  (defun remacs-test-eww-override-button (tag attrs)
    (if (eq tag 'input)
        (dom-node 'input (cons '(type . "submit") attrs))
      (dom-node 'button attrs "Send")))
  (dolist (table '(nil t))
    (dolist (tag '(button input))
      (let ((advanced (remacs-test-eww-override-button
                       tag '((id . "advanced") (name . "go") (value . "advanced")
                             (formaction . "../advanced?keep=1#results")
                             (formmethod . "PoSt")
                             (formenctype . "application/x-www-form-urlencoded"))))
            (normal (remacs-test-eww-override-button
                     tag '((id . "normal") (name . "go") (value . "normal")))))
        (remacs-test-eww-overrides
         '((action . "/parent?lang=en#parent") (method . "get"))
         (list advanced normal) '("advanced" "normal")
         '(("https://example.test/advanced?keep=1#results" "POST"
            "q=hello+world&go=advanced" (("Content-Type" . "application/x-www-form-urlencoded")))
           ("https://example.test/parent?q=hello+world&go=normal#parent" nil nil nil)) table)
        ;; An unnamed submitter still supplies all three override attributes.
        (remacs-test-eww-overrides
         '((action . "/parent") (method . "post") (enctype . "multipart/form-data"))
         (list (remacs-test-eww-override-button
                tag '((id . "unnamed") (formaction . "plain#part") (formmethod . "POST")
                      (formenctype . "TeXt/PlAiN"))))
         "unnamed"
         '("https://example.test/base/plain#part" "POST" "q=hello world\r\n"
           (("Content-Type" . "text/plain"))) table)
        ;; Missing attributes fall back independently to the owning form.
        (remacs-test-eww-overrides
         '((action . "/parent?keep=1") (method . "POST") (enctype . "text/plain"))
         (list (remacs-test-eww-override-button tag '((id . "only-action") (formaction . "next"))))
         "only-action"
         '("https://example.test/base/next" "POST" "q=hello world\r\n"
           (("Content-Type" . "text/plain"))) table)
        (remacs-test-eww-overrides
         '((action . "/parent?old=1#keep") (method . "POST") (enctype . "text/plain"))
         (list (remacs-test-eww-override-button tag '((id . "only-method") (formmethod . "GET"))))
         "only-method"
         '("https://example.test/parent?q=hello+world#keep" nil nil nil) table))))
  (dolist (override '("" "invalid"))
    (remacs-test-eww-overrides
     '((action . "/parent") (method . "post") (enctype . "multipart/form-data"))
     (list (dom-node 'button (list '(id . "empty") (cons 'formaction "")
                                  (cons 'formmethod override) (cons 'formenctype override)) "Send"))
     "empty" '("https://example.test/base/page?q=hello+world#old" nil nil nil))
    (remacs-test-eww-overrides
     '((action . "/parent") (method . "post") (enctype . "multipart/form-data"))
     (list (dom-node 'button (list '(id . "encoding") (cons 'formenctype override)) "Send"))
     "encoding" '("https://example.test/parent" "POST" "q=hello+world"
                   (("Content-Type" . "application/x-www-form-urlencoded")))))
  (remacs-test-eww-overrides
   '((action . "/parent") (method . "post"))
   (list (dom-node 'button '((id . "empty-action") (formaction . "")) "Send"))
   "empty-action" '("https://example.test/base/page?old=1#old" "POST" "q=hello+world"
                     (("Content-Type" . "application/x-www-form-urlencoded"))))
  ;; Default submitter selection is independent of the focused field and name.
  (remacs-test-eww-overrides
   '((action . "/parent") (method . "get"))
   (list (dom-node 'button '((id . "first") (formaction . "/default") (formmethod . "post")) "First")
         (dom-node 'button '((id . "second") (name . "go") (value . "second")) "Second"))
   "after" '("https://example.test/default" "POST" "q=hello+world&tail=end"
             (("Content-Type" . "application/x-www-form-urlencoded")))
   nil (list (dom-node 'input '((id . "after") (name . "tail") (value . "end")))))
  (remacs-test-eww-overrides
   '((action . "/parent") (method . "get"))
   (list (dom-node 'button '((id . "disabled") (disabled . "") (formaction . "/blocked")) "Blocked")
         (dom-node 'button '((id . "second") (formaction . "/second")) "Second"))
   "q" nil)
  (remacs-test-eww-overrides
   '((action . "/parent") (method . "get"))
   (list (dom-node 'button '((id . "dialog") (formmethod . "dialog")) "Close"))
   "dialog" nil nil (list (dom-node 'input '((id . "upload") (type . "file") (name . "upload"))))
   (lambda () (plist-put (cdr (remacs-test-eww-override-field "upload")) :filename
                         "/private/tmp/remacs-do-not-read-dialog-2670")))
  (remacs-test-eww-overrides
   '((action . "/parent") (method . "post"))
   (list (dom-node 'button '((id . "plain") (formenctype . "text/plain")) "Plain"))
   "plain" '("https://example.test/parent" "POST"
              "q=hello world\r\na=b&c=one\r\ntwo\r\nthree\r\nfour=4\r\n"
              (("Content-Type" . "text/plain")))
   nil (list (dom-node 'input '((type . "hidden") (name . "a=b&c")
                                (value . "one\ntwo\rthree\r\nfour=4")))))
  (remacs-test-eww-overrides
   '((action . "/parent") (method . "post"))
   (list (dom-node 'button '((id . "multipart") (formenctype . "multipart/form-data")) "Multipart"))
   "multipart"
   (lambda (request)
     (and (equal (nth 1 request) "POST")
          (string-prefix-p "multipart/form-data; boundary=" (cdr (assoc "Content-Type" (nth 3 request))))
          (string-search "name=\"q\"" (nth 2 request))
          (string-search "hello world" (nth 2 request)))))
  (let ((file (make-temp-file "remacs-eww-overrides-")))
    (unwind-protect
        (progn
          (with-temp-file file (insert "file-contents-2670"))
          (dolist (encoding '(nil "application/x-www-form-urlencoded" "text/plain"))
            (remacs-test-eww-overrides
             '((action . "/upload") (method . "post") (enctype . "multipart/form-data"))
             (list (dom-node 'button (list '(id . "send") (cons 'formenctype encoding)) "Send"))
             "send"
             (lambda (request)
               (let ((body (nth 2 request))
                     (type (cdr (assoc "Content-Type" (nth 3 request)))))
                 (and (equal (car request) "https://example.test/upload")
                      (equal (nth 1 request) "POST")
                      (if (null encoding)
                          (and (string-prefix-p "multipart/form-data; boundary=" type)
                               (string-search "file-contents-2670" body)
                               (string-search (file-name-nondirectory file) body))
                        (and (equal type encoding)
                             (not (string-search "file-contents-2670" body))
                             (equal body
                                    (if (equal encoding "text/plain")
                                        (concat "q=hello world\r\nupload=" (file-name-nondirectory file) "\r\n")
                                      (concat "q=hello+world&upload=" (file-name-nondirectory file)))))))))
             nil (list (dom-node 'input '((id . "upload") (type . "file") (name . "upload"))))
             (lambda () (plist-put (cdr (remacs-test-eww-override-field "upload")) :filename file)))))
      (delete-file file)))
  ;; Neither GET nor non-multipart POST reads file contents, even with a stale path.
  (dolist (method '("get" "post"))
    (remacs-test-eww-overrides
     (list '(action . "/file") (cons 'method method))
     (list (dom-node 'button '((id . "send")) "Send"))
     "send"
     (if (equal method "post")
         '("https://example.test/file" "POST" "q=hello+world&upload=stale-2670.bin"
           (("Content-Type" . "application/x-www-form-urlencoded")))
       '("https://example.test/file?q=hello+world&upload=stale-2670.bin" nil nil nil))
     nil (list (dom-node 'input '((id . "upload") (type . "file") (name . "upload"))))
     (lambda () (plist-put (cdr (remacs-test-eww-override-field "upload")) :filename
                           "/private/tmp/remacs-does-not-exist/stale-2670.bin"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let id = interp.intern("remacs-test-eww-override-phase");
            let phase = interp.symbol_value(id);
            panic!(
                "{} {} phase={}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase)
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn submitter_attributes_override_only_their_own_request() {
    let mut interp = Interp::new();
    eprintln!("default EWW submitter attributes");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW submitter attributes");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW submitter attributes");
    check(&mut interp, CASES);
}
