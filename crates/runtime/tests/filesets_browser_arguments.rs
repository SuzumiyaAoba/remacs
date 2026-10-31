use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'filesets)
  (let* ((program "/mock/browser with spaces")
         (directory "/private/tmp/my dir; $(literal) \"quoted\" 日本")
         (expanded (expand-file-name directory))
         (cases (list
                 (list "--select %s" (list "--select" expanded))
                 (list "\"%s\"" (list expanded))
                 (list "--select=%s --title \"My Files\""
                       (list (concat "--select=" expanded) "--title" "My Files"))
                 (list "--same %s %s" (list "--same" expanded expanded))
                 (list "\"\" %s" (list "" expanded))
                 (list "--title \"say \\\"hi\\\"\" %s" (list "--title" "say \"hi\"" expanded))
                 (list "--title \"a\\\\b\" %s" (list "--title" "a\\b" expanded))
                 (list "--literal \"%%s\" %s" (list "--literal" "%s" expanded))
                 (list " \t " nil))))
    (dolist (case cases)
      (let ((filesets-browse-dir-function (list program (car case)))
            (token (list :process)) calls)
        (cl-letf (((symbol-function 'start-process)
                   (lambda (_name _buffer command &rest arguments)
                     (push (list command arguments) calls) token)))
          (unless (and (eq (filesets-browse-dir directory) token)
                       (equal calls (list (list program (cadr case)))))
            (error "Browser template %S: expected %S, got %S" (car case) (cadr case) calls)))))
    (let* ((default-directory temporary-file-directory)
           (relative "relative directory")
           (filesets-browse-dir-function '("probe" "--select %s"))
           received)
      (cl-letf (((symbol-function 'start-process)
                 (lambda (_name _buffer _command &rest arguments) (setq received arguments))))
        (filesets-browse-dir relative))
      (unless (equal received (list "--select" (expand-file-name relative default-directory)))
        (error "Browser did not expand the relative directory"))))
  ;; Function-valued browsers receive the caller's original object.
  (let* ((directory (propertize "relative directory" 'face 'bold))
         (filesets-browse-dir-function
          (lambda (name)
            (unless (eq name directory) (error "Changed the directory object"))
            :function-result)))
    (cl-letf (((symbol-function 'start-process) (lambda (&rest _) (error "Spawned a function browser"))))
      (unless (eq (filesets-browse-dir directory) :function-result)
        (error "Lost the function browser's result"))))
  (let ((filesets-browse-dir-function '("probe" "\"unterminated")) called)
    (cl-letf (((symbol-function 'start-process) (lambda (&rest _) (setq called t))))
      (unless (and (condition-case nil (progn (filesets-browse-dir "/tmp") nil) (end-of-file t))
                   (null called))
        (error "Malformed browser quoting reached process creation"))))
  t)
"#;

#[test]
fn filesets_browser_passes_separate_arguments_without_splitting_the_directory() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
