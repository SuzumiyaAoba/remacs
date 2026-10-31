use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"(progn
  (require 'uniquify)
  (defvar remacs-3205-dead nil)
  (defvar remacs-3205-holder nil)
  (defvar remacs-3205-newbuf nil)
  (defvar remacs-3205-item nil)
  (let ((uniquify-after-kill-buffer-flag nil))
    (setq remacs-3205-dead (generate-new-buffer " *remacs-3205-dead*"))
    (setq remacs-3205-holder (generate-new-buffer " *remacs-3205-holder*"))
    (setq remacs-3205-newbuf (generate-new-buffer " *remacs-3205-new*"))
    (unwind-protect
        (progn
          (setq remacs-3205-item
                (uniquify-make-item "base" "/old" remacs-3205-dead))
          (with-current-buffer remacs-3205-holder
            (setq-local uniquify-managed (list remacs-3205-item)))
          (kill-buffer remacs-3205-dead)
          (with-current-buffer remacs-3205-newbuf
            (uniquify-rationalize-file-buffer-names
             "base" "/new" remacs-3205-newbuf))
          (unless (and (equal (buffer-name remacs-3205-newbuf) "base")
                       (null (buffer-local-value
                              'uniquify-managed remacs-3205-holder)))
            (error "Stale dead-buffer items were not safely removed")))
      (dolist (buffer (list remacs-3205-dead
                            remacs-3205-holder
                            remacs-3205-newbuf))
        (when (buffer-live-p buffer)
          (kill-buffer buffer)))))
  t)"#;

#[test]
fn rationalization_discards_dead_items_before_reading_the_head() {
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
