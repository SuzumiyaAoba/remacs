use remacs_runtime::lisp::{Flow, Interp, Value};

const CASES: &str = r#"
(let ((text-quoting-style 'straight))
  (defun remacs-test-transpose (command text start mark-pos &optional bounds arg)
    (with-temp-buffer
      (insert text)
      (when bounds (apply #'narrow-to-region bounds))
      (goto-char start)
      (when mark-pos (set-mark mark-pos))
      (setq mark-active t)
      (condition-case err
          (let ((value (funcall command (or arg 0))))
            (when (and (= (or arg 0) 0) value)
              (error "ARG=0 must retain its nil return value"))
            (list (buffer-string) (point)))
        (error (list (car err) (cadr err) (buffer-string) (point))))))
  (mapcar
   (lambda (case) (apply #'remacs-test-transpose case))
   '((transpose-words "one two" 5 1)
     (transpose-words "one two" 1 5)
     (transpose-words "short longerword" 7 1)
     (transpose-words "short longerword" 1 7)
     (transpose-words "short longerword" 9 3)
     (transpose-lines "one\ntwo\n" 5 1)
     (transpose-lines "one\ntwo\n" 1 5)
     (transpose-lines "aa\nmid\nbbbb\n" 8 1)
     (transpose-lines "aa\nmid\nbbbb\n" 1 8)
     (transpose-lines "one\ntwo" 5 1)
     (transpose-chars "abcde" 4 1)
     (transpose-chars "abcde" 1 4)
     (transpose-chars "A日B😀C" 4 2)
     (transpose-sexps "(a) (b c)" 5 1)
     (transpose-sexps "(a) (b c)" 1 5)
     ((lambda (_arg)
        (transpose-subr (lambda (_n) (cons (point) (1+ (point)))) 0 t))
      "abcde" 4 1)
     (transpose-words "<one two>" 6 2 (2 9))
     (transpose-words "<one two>" 2 6 (2 9))
     (transpose-words "one two" 4 1 nil 1)
     (transpose-words "one two" 8 1 nil -1)
     (transpose-words "one two" 5 nil)
     (transpose-words "one two" 2 3)
     ((lambda (_arg) (transpose-subr (lambda (_n) nil) 0 t)) "ab" 1 2))))
"#;

const EXPECTED: &str = r#"
'(("two one" 4)
  ("two one" 8)
  ("longerword short" 11)
  ("longerword short" 17)
  ("longerword short" 11)
  ("two\none\n" 5)
  ("two\none\n" 9)
  ("bbbb\nmid\naa\n" 6)
  ("bbbb\nmid\naa\n" 13)
  ("two\none\n" 5)
  ("dbcae" 2)
  ("dbcae" 5)
  ("A😀B日C" 3)
  ("(b c) (a)" 6)
  ("(b c) (a)" 10)
  ("dbcae" 2)
  ("two one" 5)
  ("two one" 9)
  ("two one" 8)
  ("two one" 4)
  (error "No mark set in this buffer" "one two" 5)
  (error "Don't have two things to transpose" "one two" 2)
  (error "Don't have two things to transpose" "ab" 1))
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

#[test]
fn zero_argument_finishes_after_the_moved_object_in_both_definitions() {
    let mut interp = Interp::new();
    let expected = eval(&mut interp, EXPECTED);
    let expected = interp.prin1_to_string(&expected);
    let actual = eval(&mut interp, CASES);
    assert_eq!(
        interp.prin1_to_string(&actual),
        expected,
        "bootstrap definition"
    );

    // Read the actual shipped definition as well as the folded bootstrap one.
    let source = remacs_libraries::EMBEDDED_LISP
        .iter()
        .find(|(name, _)| *name == "simple")
        .unwrap()
        .1;
    let start = source.find("(defun transpose-subr (").unwrap();
    let definition = source[start..]
        .split_once("\n(defun transpose-subr-1 (")
        .unwrap()
        .0;
    let quoted = interp.prin1_to_string(&Value::string(definition));
    eval(&mut interp, &format!("(eval (read {quoted}) t)"));
    let actual = eval(&mut interp, CASES);
    assert_eq!(
        interp.prin1_to_string(&actual),
        expected,
        "shipped definition"
    );
}
