use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'xml)
  (dolist (name '(xml--parse-buffer xml--parse-processing-instruction
                  xml-parse-tag-1 xml--check-entity-recursion xml-parse-string
                  xml-remove-comments))
    (unless (fboundp name) (error "Missing XML definition: %S" name)))
  (defun remacs-test-xml-parse (text &optional namespaces)
    (with-temp-buffer
      (insert text)
      (xml-parse-region nil nil nil nil namespaces)))
  (dolist (case '(("<root/>" ((root nil)))
                  (" \n<root><child a='1'>text</child></root> \n"
                   ((root nil (child ((a . "1")) "text"))))
                  ("<?xml version='1.0'?><?style href='x'?><root/><!--tail-->"
                   ((root nil)))
                  ("<root><?style href='x'?><child/></root>"
                   ((root nil (child nil))))
                  ("<root>x<!--comment--> y</root>" ((root nil "x y")))
                  ("<root><![CDATA[x<y]]>&amp;&#65;</root>"
                   ((root nil "x<y&A")))))
    (let ((actual (remacs-test-xml-parse (car case))))
      (unless (equal actual (cadr case))
        (error "XML %S: expected %S, got %S" (car case) (cadr case) actual))))
  (let ((actual (remacs-test-xml-parse
                 "<p:root p:a='v' xmlns:p='urn:r'><p:child/></p:root>" t))
        (expected '((("urn:r" . "root")
                     ((("urn:r" . "a") . "v")
                      (("http://www.w3.org/2000/xmlns/" . "p") . "urn:r"))
                     (("urn:r" . "child") nil)))))
    (unless (equal actual expected)
      (error "Namespace scope: expected %S, got %S" expected actual)))
  (dolist (text '("outside<root/>" "<root/>outside" "<root/><other/>"
                  "<?XML version='1.0'?><root/>" "<root><?xml version='1.0'?></root>"
                  "<?pi unfinished<root/>" "<root><child></root>"))
    (unless (condition-case nil (progn (remacs-test-xml-parse text) nil)
              (error t))
      (error "Malformed XML was accepted: %S" text)))
  (let ((xml-entity-alist '(("a" . "&b;&b;") ("b" . "B"))))
    (unless (member "b" (xml--check-entity-recursion "a"))
      (error "Entity traversal did not return the visited names")))
  (let ((xml-entity-alist '(("a" . "&b;") ("b" . "&a;"))))
    (unless (condition-case nil (progn (xml--check-entity-recursion "a") nil)
              (error t))
      (error "Recursive entities were accepted")))
  t)
"#;

#[test]
fn xml_load_installs_independent_definitions_and_parses_documents() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(result) => assert!(result.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
