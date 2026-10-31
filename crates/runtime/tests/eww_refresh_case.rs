use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'eww)
  (require 'cl-lib)
  (dolist (folding '(nil t))
    (let ((case-fold-search folding)
          (shr-base "https://base.invalid/path/"))
      (dolist (keyword '("url" "URL" "uRl"))
        (dolist (delimiter '("" "\"" "'"))
          (dolist (timeout '("0" "1" "2"))
            (let* ((target "https://example.test/Case%2FPath?q=V#Frag")
                   (content (concat timeout "; " keyword "=" delimiter target delimiter))
                   (eww-redirect-level 1)
                   observed)
              (cl-letf (((symbol-function 'eww)
                         (lambda (url)
                           (setq observed
                                 (list 'redirect url case-fold-search eww-redirect-level))))
                        ((symbol-function 'eww-tag-a)
                         (lambda (dom)
                           (setq observed
                                 (list 'link (dom-attr dom 'href) (car (dom-children dom))
                                       case-fold-search eww-redirect-level)))))
                (eww-tag-meta
                 (dom-node 'meta
                           (list (cons 'http-equiv "ReFrEsH") (cons 'content content)))))
              (unless (and (equal observed
                                  (if (equal timeout "0")
                                      (list 'redirect target folding 2)
                                    (list 'link target
                                          (if (equal timeout "1")
                                              "Auto refresh in 1 second disabled"
                                            "Auto refresh in 2 seconds disabled")
                                          folding 2)))
                           (eq case-fold-search folding) (= eww-redirect-level 1))
                (error "Refresh %S failed or changed callback bindings with folding=%S: %S"
                       content folding observed))))))
      (dolist (case '((5 "refresh" "0; URL=https://example.test/next")
                      (1 "refreshx" "0; URL=https://example.test/next")
                      (1 "refresh" "invalid")
                      (1 "refresh" nil)))
        (let ((eww-redirect-level (nth 0 case)))
          (cl-letf (((symbol-function 'eww)
                     (lambda (&rest _) (error "Ignored meta tag triggered navigation")))
                    ((symbol-function 'eww-tag-a)
                     (lambda (&rest _) (error "Ignored meta tag inserted a link"))))
            (eww-tag-meta
             (dom-node 'meta
                       (list (cons 'http-equiv (nth 1 case))
                             (cons 'content (nth 2 case))))))))))
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
fn refresh_url_keywords_ignore_caller_search_case_folding() {
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
    let start = source.find("(defun eww-tag-meta ").unwrap();
    let end = source[start..]
        .find("(defun eww-tag-link ")
        .unwrap()
        + start;
    check(&mut interp, &source[start..end]);
    check(&mut interp, CASES);
}
