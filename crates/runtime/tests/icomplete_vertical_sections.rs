use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'icomplete)
  (defun remacs-test-vertical-render (comps height &optional metadata past indicators)
    (let ((icomplete-prospects-height height)
          (icomplete-scroll (and past t))
          (icomplete--scrolled-completions (and past comps))
          (icomplete--scrolled-past past)
          (icomplete--in-region-buffer nil)
          (icomplete-vertical-render-prefix-indicator indicators)
          (icomplete-vertical-selected-prefix-indicator "> ")
          (icomplete-vertical-unselected-prefix-indicator "  ")
          (completion-lazy-hilit nil))
      (cl-letf (((symbol-function 'max-mini-window-lines) (lambda () 20)))
        (icomplete--render-vertical comps metadata))))
  (defun remacs-test-vertical-equal (label actual expected)
    (unless (equal actual expected)
      (error "Vertical completion %S: expected %S, got %S" label expected actual)))
  (let ((completions-group t))
    (dolist (case '((2 " \nfoo\n") (3 " \nfoo\nbar\n")
                    (4 " \nfoo\nbar\nbaz\n") (10 " \nfoo\nbar\nbaz\n")
                    (1 " \n") (0 " \n") (-1 " \n")))
      (remacs-test-vertical-equal
       (car case) (remacs-test-vertical-render '("foo" "bar" "baz") (car case))
       (cadr case)))
    (remacs-test-vertical-equal 'dotted-tail
      (remacs-test-vertical-render '("foo" "bar" . 3) 3) " \nfoo\nbar\n")
    (remacs-test-vertical-equal 'scroll-above
      (remacs-test-vertical-render '("selected" . 0) 4 nil '("near" "far"))
      " \nfar\nnear\nselected\n")
    (let* ((candidate (propertize "foo" 'face 'bold))
           (output (remacs-test-vertical-render (list candidate "bar") 3 nil nil t))
           (position (string-match "foo" output)))
      (remacs-test-vertical-equal 'indicators output " \n> foo\n  bar\n")
      (unless (and position (get-text-property position 'icomplete-selected output)
                   (get-text-property position 'face output)
                   (null (get-text-property 0 'icomplete-selected candidate))
                   (eq (get-text-property 0 'face candidate) 'bold))
        (error "Vertical rendering lost selection properties or changed its input")))
    (let ((metadata (list 'metadata (cons 'annotation-function (lambda (_) " [doc]")))))
      (remacs-test-vertical-equal 'annotations
        (remacs-test-vertical-render '("foo" "bar") 3 metadata)
        " \nfoo [doc]\nbar [doc]\n"))
    (let ((metadata (list 'metadata (cons 'group-function (lambda (&rest _) nil)))))
      (remacs-test-vertical-equal 'nil-groups
        (remacs-test-vertical-render '("foo" "bar") 2 metadata) " \nfoo\n"))
    (let* ((metadata (list 'metadata
                           (cons 'group-function
                                 (lambda (comp transform)
                                   (unless transform (upcase (substring comp 0 1)))))))
           (output (remacs-test-vertical-render '("a1" "a2" "b1") 10 metadata)))
      (remacs-test-vertical-equal 'grouped output " \nA\na1\na2\nB\nb1\n")
      (unless (eq (get-text-property 2 'face output) 'icomplete-section)
        (error "Group header lost its section face"))
      (remacs-test-vertical-equal 'group-scroll
        (remacs-test-vertical-render '("selected" . 0) 3 metadata '("near"))
        " \nS\nselected\n")
      (let ((completions-group nil))
        (remacs-test-vertical-equal 'disabled-groups
          (remacs-test-vertical-render '("a1" "b1") 2 metadata) " \na1\n"))))
  t)
"#;

#[test]
fn vertical_completion_renders_without_sections_at_small_heights() {
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
