use remacs_runtime::lisp::{Flow, Interp, Value};

const CASES: &str = r#"
(progn
  (require 'seq)
  (when (seq-contains '("foobar") "foo" #'string-prefix-p)
    (error "seq-contains reversed the prefix predicate arguments"))
  (let ((contains (symbol-function 'seq-contains)))
    (dolist (factory '(list vector))
      (let* ((element (propertize "foo" 'face 'bold))
             (sequence (funcall factory element "later")))
        (unless (and (eq (funcall contains sequence "foobar" #'string-prefix-p) element)
                     (seq-contains-p sequence "foobar" #'string-prefix-p)
                     (= (seq-position sequence "foobar" #'string-prefix-p) 0))
          (error "Sequence APIs disagree on element-first comparison: %S" factory)))
      (let* ((element (copy-sequence "chosen"))
             (probe (make-symbol "probe"))
             (sequence (funcall factory "skip" element "later"))
             calls)
        (unless (eq (funcall contains sequence probe
                            (lambda (candidate wanted)
                              (unless (eq wanted probe) (error "Changed the search object"))
                              (push candidate calls)
                              (when (eq candidate element) :matched)))
                    element)
          (error "seq-contains returned the predicate result instead of the element"))
        (unless (equal (nreverse calls) (list "skip" element))
          (error "seq-contains did not stop at the first match")))
      (let ((calls 0))
        (unless (and (null (funcall contains (funcall factory nil "later") 'needle
                                   (lambda (_element _target) (setq calls (1+ calls)) t)))
                     (= calls 1))
          (error "Matching nil did not stop the search"))))
    (unless (= (funcall contains "abc" ?b #'<) ?a)
      (error "String sequence comparison used the wrong argument order"))
    (unless (= (funcall contains '(1 2 3) 2 #'<) 1)
      (error "Numeric comparison used the wrong argument order"))
    (let ((element (list 'a 'b)))
      (unless (and (eq (funcall contains (list element) '(a b)) element)
                   (eq (funcall contains (vector element) '(a b) nil) element))
        (error "Default equality or element identity changed")))
    (dolist (sequence (list nil [] ""))
      (when (funcall contains sequence 'needle (lambda (&rest _) (error "Compared an empty sequence")))
        (error "Empty sequence returned a match")))
    (unless (eq (catch 'remacs-seq-stop
                  (funcall contains '(1) 2 (lambda (&rest _) (throw 'remacs-seq-stop :stopped))))
                :stopped)
      (error "seq-contains swallowed a nonlocal exit"))
    (unless (equal (condition-case err
                       (funcall contains '(1) 2 (lambda (&rest _) (error "Comparison failed")))
                     (error err))
                   '(error "Comparison failed"))
      (error "seq-contains swallowed a comparison error")))
  t)
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
fn obsolete_seq_contains_calls_the_predicate_element_first() {
    let mut interp = Interp::new();
    assert!(eval(&mut interp, CASES).truthy());
    eval(&mut interp, "(require 'cl-lib)");
    eval(
        &mut interp,
        "(load (let ((load-path nil)) (locate-library \"seq\")) nil t t)",
    );
    assert!(eval(&mut interp, CASES).truthy());

    let dumped = include_str!("../../../lisp/subr-x.el");
    let start = dumped.find("(defun seq-contains ").unwrap();
    let end = dumped[start..]
        .find("\n(defun seq-set-equal-p ")
        .unwrap()
        + start;
    eval(&mut interp, &format!("(eval '{} t)", &dumped[start..end]));
    assert!(eval(&mut interp, CASES).truthy());
}
