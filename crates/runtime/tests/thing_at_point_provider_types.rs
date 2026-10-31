use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'thingatpt)
  (defvar remacs-thing-list-value nil)
  (defvar remacs-thing-vector-value nil)
  (defvar remacs-thing-bool-vector-value nil)
  (defvar remacs-thing-string-value nil)
  (defun remacs-thing-list-provider () remacs-thing-list-value)
  (defun remacs-thing-vector-provider () remacs-thing-vector-value)
  (defun remacs-thing-bool-vector-provider () remacs-thing-bool-vector-value)
  (defun remacs-thing-string-provider () remacs-thing-string-value)
  (let ((thing-at-point-provider-alist
         '((list-probe . remacs-thing-list-provider)
           (vector-probe . remacs-thing-vector-provider)
           (bool-vector-probe . remacs-thing-bool-vector-provider))))
    (let ((remacs-thing-list-value '(one two))
          (remacs-thing-vector-value [one two])
          (remacs-thing-bool-vector-value (make-bool-vector 3 t)))
      (unless (and (eq remacs-thing-list-value
                       (thing-at-point 'list-probe t))
                   (eq remacs-thing-list-value
                       (thing-at-point 'list-probe))
                   (eq remacs-thing-vector-value
                       (thing-at-point 'vector-probe t))
                   (eq remacs-thing-vector-value
                       (thing-at-point 'vector-probe))
                   (eq remacs-thing-bool-vector-value
                       (thing-at-point 'bool-vector-probe t))
                   (eq remacs-thing-bool-vector-value
                       (thing-at-point 'bool-vector-probe)))
        (error "NO-PROPERTIES changed a non-string provider result"))))
  (let* ((source (propertize "text" 'face 'bold))
         (remacs-thing-string-value source)
         (thing-at-point-provider-alist
          '((string-probe . remacs-thing-string-provider)))
         (result (thing-at-point 'string-probe t)))
    (unless (and (string-equal result "text")
                 (not (eq result source))
                 (null (get-text-property 0 'face result))
                 (eq (get-text-property 0 'face source) 'bold)
                 (eq (thing-at-point 'string-probe) source))
      (error "NO-PROPERTIES did not copy and strip string properties")))
  t)
"#;

#[test]
fn thing_at_point_no_properties_preserves_nonstring_provider_values() {
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
