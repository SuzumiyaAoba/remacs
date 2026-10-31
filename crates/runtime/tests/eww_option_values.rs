use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (defun remacs-test-eww-option (method options expected &optional choice table)
    (with-temp-buffer
      (let* ((eww-data (list :url "https://example.test/page"))
             (shr-use-fonts nil)
             (shr-width 100)
             (shr-external-rendering-functions
              '((form . eww-tag-form) (select . eww-tag-select) (button . eww-form-submit)))
             (select (append (list 'select '((name . "color"))) options))
             (button (dom-node 'button nil "Send"))
             (form (append (list 'form (list (cons 'method method)
                                            (cons 'action "https://example.test/search")))
                           (if table
                               (list (dom-node 'table nil (dom-node 'tr nil
                                                    (dom-node 'td nil select)
                                                    (dom-node 'td nil button))))
                             (list select button))))
             field start observed)
        (shr-insert-document (list 'html nil (list 'body nil form)))
        (goto-char (point-min))
        (while (and (< (point) (point-max)) (not start))
          (let ((input (get-text-property (point) 'eww-form)))
            (when (equal (plist-get input :name) "color")
              (setq field input start (point))))
          (unless start (forward-char)))
        (unless start (error "Option produced no select field"))
        (when choice
          (goto-char start)
          (cl-letf (((symbol-function 'mouse-set-point) #'ignore)
                    ((symbol-function 'popup-menu)
                     (lambda (menu &rest _)
                       (call-interactively (aref (nth (1+ choice) menu) 1)))))
            (eww-change-select nil)))
        (goto-char (point-min))
        (while (and (< (point) (point-max))
                    (not (equal (plist-get (get-text-property (point) 'eww-form) :type) "submit")))
          (forward-char))
        (when (eobp) (error "Form produced no submit button"))
        (cl-letf (((symbol-function 'eww-browse-url)
                   (lambda (url &rest _)
                     (setq observed (list url url-request-method url-request-data)))))
          (eww-submit))
        (let* ((query (if (equal method "post") (nth 2 observed)
                        (cdr (url-path-and-query (url-generic-parse-url (car observed))))))
               (entries (url-parse-query-string (string-replace "+" "%20" query) nil t))
               (value (and (= (length entries) 1)
                           (equal (caar entries) "color")
                           (decode-coding-string (cadar entries) 'utf-8))))
          (unless (and (equal (plist-get field :value) expected) (equal value expected))
            (error "Option value %S was not submitted (%S table=%S choice=%S): %S / %S"
                   expected method table choice (plist-get field :value) observed))))))
  (dolist (method '("get" "post"))
    (dolist (table '(nil t))
      (dolist (case
               (list
                (list (list (dom-node 'option nil "red") (dom-node 'option nil "blue")) "red" nil)
                (list (list (dom-node 'option nil "red") (dom-node 'option nil "blue")) "blue" 1)
                (list (list (dom-node 'option nil "red")
                            (dom-node 'option '((selected . "")) "blue")) "blue" nil)
                (list (list (dom-node 'option '((value . "")) "red")) "" nil)
                (list (list (dom-node 'option '((value . "  raw\t ")) "red")) "  raw\t " nil)
                (list (list (dom-node 'option nil " \t red\nblue\r\f ")) "red blue" nil)
                (list (list (dom-node 'option nil "r" (dom-node 'b nil "e") "d")) "red" nil)
                (list (list (dom-node 'option nil "red" (dom-node 'script nil "ignored")
                                     (dom-node 'comment nil "ignored") "blue")) "redblue" nil)
                (list (list (dom-node 'option nil "red"
                                     (dom-node 'img '((alt . "ignored")) "ignored")
                                     (dom-node 'template nil "ignored") "blue")) "redblue" nil)
                (list (list (dom-node 'option nil "")) "" nil)
                (list (list (dom-node 'option nil "red") (dom-node 'option nil "")) "" 1)
                (list (list (dom-node 'option nil " red ")) " red " nil)
                (list (list (dom-node 'option nil "日本語")) "日本語" nil)
                (list (list (dom-node 'option '((value . "keep")) "red")) "keep" nil)))
        (remacs-test-eww-option method (nth 0 case) (nth 1 case) (nth 2 case) table))))
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
fn options_without_values_submit_normalized_text() {
    let mut interp = Interp::new();
    eprintln!("default EWW option value rendering and submission");
    check(&mut interp, CASES);
    check(
        &mut interp,
        &format!(
            "(load {:?} nil t)",
            concat!(env!("REMACS_SOURCE_ROOT"), "/lisp-elc/eww.elc")
        ),
    );
    eprintln!("AOT EWW option value rendering and submission");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "eww.el" nil t)"#);
    eprintln!("source EWW option value rendering and submission");
    check(&mut interp, CASES);
}
