use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'ring)
  (let ((ring (make-ring 0)))
    (ring-insert-at-beginning ring 'item)
    (unless (and (ring-p ring) (= (ring-size ring) 0)
                 (= (ring-length ring) 0) (ring-empty-p ring)
                 (null (ring-elements ring)))
      (error "ring-insert-at-beginning changed a zero-capacity ring")))
  (let ((ring (make-ring 2)))
    (ring-insert ring 'a)
    (ring-insert ring 'b)
    (ring-insert-at-beginning ring 'oldest)
    (unless (and (= (ring-size ring) 2)
                 (= (ring-length ring) 2)
                 (equal (ring-elements ring) '(a oldest)))
      (error "ring-insert-at-beginning changed full-ring semantics")))
  (let ((ring (make-ring 2)))
    (ring-insert ring 'a)
    (ring-insert ring 'b)
    (ring-resize ring 0)
    (ring-insert-at-beginning ring 'after-resize)
    (unless (and (= (ring-size ring) 0)
                 (= (ring-length ring) 0)
                 (ring-empty-p ring))
      (error "ring-insert-at-beginning corrupted a zero ring after resize")))
  t)
"#;

#[test]
fn ring_insert_at_beginning_on_zero_capacity_is_a_safe_noop() {
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
