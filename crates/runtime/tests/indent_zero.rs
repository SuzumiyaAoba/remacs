use remacs_runtime::lisp::{Flow, Interp, Value};

const ZERO_CASES: &str = r#"
(progn
  (defun remacs-test-zero-indent
      (text tabs arg modified readonly &optional bounds marker-bounds)
    (with-temp-buffer
      (insert text)
      (when bounds (apply #'narrow-to-region bounds))
      (setq indent-tabs-mode tabs tab-width 8)
      (unless (= (point-min) (point-max))
        (put-text-property (point-min) (1+ (point-min)) 'indent-zero-test t))
      (goto-char (min (point-max) (1+ (point-min))))
      (set-mark (point-min))
      (setq mark-active t)
      (buffer-enable-undo)
      (setq buffer-undo-list nil)
      (set-buffer-modified-p modified)
      (setq buffer-read-only readonly)
      (let ((before 0) (after 0)
            (original (buffer-string)) (undo buffer-undo-list)
            (tick (buffer-modified-tick))
            (chars-tick (buffer-chars-modified-tick))
            (old-point (point)) (old-mark (mark))
            (old-min (point-min)) (old-max (point-max))
            (start (if marker-bounds (copy-marker (point-min)) (point-min)))
            (end (if marker-bounds (copy-marker (point-max)) (point-max))))
        (add-hook 'before-change-functions
                  (lambda (&rest _) (setq before (1+ before))) nil t)
        (add-hook 'after-change-functions
                  (lambda (&rest _) (setq after (1+ after))) nil t)
        (unwind-protect
            (let ((result (indent-rigidly start end arg)))
              (list (null result)
                    (equal-including-properties original (buffer-string))
                    (eq modified (buffer-modified-p))
                    (eq undo buffer-undo-list)
                    (= tick (buffer-modified-tick))
                    (= chars-tick (buffer-chars-modified-tick))
                    (= old-point (point)) (= old-mark (mark))
                    (= before 0) (= after 0)
                    (= old-min (point-min)) (= old-max (point-max))
                    (eq readonly buffer-read-only)
                    (or (not marker-bounds)
                        (and (= (marker-position start) old-min)
                             (= (marker-position end) old-max)))))
          (when marker-bounds (set-marker start nil) (set-marker end nil))))))
  (mapcar
   (lambda (case) (apply #'remacs-test-zero-indent case))
   '(("\tfoo\n" nil 0 nil nil)
     ("        foo\n" t 0 nil nil)
     ("\t  foo\n  \tbar\n" nil 0 nil nil)
     (" \t\n\t\n" t 0 nil nil)
     ("" nil 0 nil nil)
     ("\tfoo\n" nil 0 t nil)
     ("\tfoo\n" nil 0 nil t)
     ("<\tfoo\n>" nil 0 nil nil (2 7) t)
     ("\tfoo\n" nil (0) nil nil)
     ("\tfoo\n" nil (0 . ignored) nil nil)
     ("\tfoo\n" nil 0 nil nil nil t))))
"#;

const CONTROLS: &str = r#"
(progn
  (require 'cl-lib)
  (list
   (with-temp-buffer
     (insert "aa\nbb\n")
     (indent-rigidly (point-min) (point-max) 2)
     (buffer-string))
   (with-temp-buffer
     (insert "  aa\n bb\n")
     (indent-rigidly (point-min) (point-max) -1)
     (buffer-string))
   (with-temp-buffer
     (condition-case err (indent-rigidly "bad" 1 0)
       (error err)))
   (with-temp-buffer
     (condition-case err (indent-rigidly 1 "end" 0)
       (error err)))
   (let (called)
     (cl-letf (((symbol-function 'set-transient-map)
                (lambda (&rest args)
                  (setq called (eq (car args) indent-rigidly-map))
                  'activated)))
       (list (with-temp-buffer (indent-rigidly 1 1 nil t)) called)))))
"#;

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

fn check(interp: &mut Interp, label: &str) {
    let result = eval(interp, ZERO_CASES);
    let rows = result.list_to_vec().unwrap();
    assert_eq!(rows.len(), 11);
    let expected = format!("({})", vec!["t"; 14].join(" "));
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(
            interp.prin1_to_string(row),
            expected,
            "{label}: case {index}"
        );
    }
    let actual = eval(interp, CONTROLS);
    let expected = eval(
        interp,
        r#"'("  aa\n  bb\n" " aa\nbb\n"
       (wrong-type-argument integer-or-marker-p "bad")
       (wrong-type-argument integer-or-marker-p "end")
       (activated t))"#,
    );
    assert_eq!(
        interp.prin1_to_string(&actual),
        interp.prin1_to_string(&expected),
        "{label}: controls"
    );
}

#[test]
fn zero_shift_preserves_text_and_all_editing_state_in_both_definitions() {
    let mut interp = Interp::new();
    check(&mut interp, "bootstrap");
    let source = remacs_libraries::EMBEDDED_LISP
        .iter()
        .find(|(name, _)| *name == "indent")
        .unwrap()
        .1;
    let start = source.find("(defun indent-rigidly (").unwrap();
    let definition = source[start..]
        .split_once("\n(defun indent-rigidly--pop-undo (")
        .unwrap()
        .0;
    let quoted = interp.prin1_to_string(&Value::string(definition));
    eval(&mut interp, &format!("(eval (read {quoted}) t)"));
    check(&mut interp, "shipped source");
}
