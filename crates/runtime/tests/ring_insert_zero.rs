use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'ring)
  (let ((ring (make-ring 0)))
    (unless (and (ring-p ring) (= (ring-size ring) 0)
                 (= (ring-length ring) 0) (ring-empty-p ring))
      (error "make-ring 0 must create an empty ring"))
    (unless (eq (ring-insert ring 'item) 'item)
      (error "ring-insert must return the inserted item"))
    (unless (and (= (ring-size ring) 0) (= (ring-length ring) 0)
                 (ring-empty-p ring) (null (ring-elements ring)))
      (error "ring-insert changed a zero-capacity ring"))
    (ring-insert ring 'another-item)
    (unless (and (= (ring-size ring) 0) (= (ring-length ring) 0)
                 (ring-empty-p ring))
      (error "Repeated ring-insert changed a zero-capacity ring")))
  (let ((ring (make-ring 0)))
    (unless (eq (ring-insert+extend ring 'discarded) 'discarded)
      (error "ring-insert+extend did not return its item"))
    (unless (and (= (ring-size ring) 0) (ring-empty-p ring))
      (error "ring-insert+extend grew without GROW-P"))
    (ring-insert+extend ring 'kept t)
    (unless (and (= (ring-size ring) 1) (= (ring-length ring) 1)
                 (equal (ring-elements ring) '(kept)))
      (error "GROW-P did not extend an empty zero-capacity ring")))
  (let ((ring (make-ring 2)))
    (ring-insert ring 'a)
    (ring-insert ring 'b)
    (unless (eq (ring-insert ring 'c) 'c)
      (error "ring-insert lost its return value when full"))
    (unless (equal (ring-elements ring) '(c b))
      (error "ring-insert changed full-ring eviction semantics")))
  (let ((ring (make-ring 2)))
    (ring-insert ring 'a)
    (ring-insert ring 'b)
    (ring-resize ring 0)
    (unless (and (= (ring-size ring) 0) (= (ring-length ring) 0))
      (error "ring-resize 0 did not make an empty zero-capacity ring"))
    (unless (eq (ring-insert ring 'after-resize) 'after-resize)
      (error "ring-insert lost its return value after zero resize"))
    (unless (and (= (ring-size ring) 0) (ring-empty-p ring))
      (error "ring-insert corrupted a zero-capacity ring after resize")))
  t)
"#;

#[test]
fn ring_insert_on_zero_capacity_is_a_safe_noop() {
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
