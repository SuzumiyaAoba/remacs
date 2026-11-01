use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-1687-check-margins (label window left right)
    (let ((margins (window-margins window)))
      (unless (and (= (or (car margins) 0) left)
                   (= (or (cdr margins) 0) right))
        (error "Linum #1687 %S: expected margins %S, got %S"
               label (cons left right) margins))))
  (defun remacs-1687-fail-margin-setter (&rest _args)
    (error "Test-only set-window-margins failure"))
  (unless (advice-member-p #'linum--forget-margin-ownership
                           'set-window-margins)
    (error "Linum margin advice was not installed"))
  ;; With no intervening external setter, Linum restores only the left margin
  ;; it added and preserves the pre-existing right margin.
  (save-window-excursion
    (let ((buffer (generate-new-buffer " *linum-1687-owned*"))
          (window (selected-window)))
      (unwind-protect
          (progn
            (set-window-buffer window buffer)
            (with-current-buffer buffer
              (set-window-margins window 0 7)
              (linum--set-window-margins window 4 7)
              (linum-delete-overlays)
              (remacs-1687-check-margins 'ordinary-owner-restoration window 0 7)))
        (when (buffer-live-p buffer)
          (kill-buffer buffer)))))
  ;; Same-value external changes invalidate ownership; failed changes do not.
  (save-window-excursion
    (let ((buffer (generate-new-buffer " *linum-1687-same-value*"))
          (window (selected-window)))
      (unwind-protect
          (progn
            (set-window-buffer window buffer)
            (with-current-buffer buffer
              (set-window-margins window 0 0)
              (linum--set-window-margins window 4 0)
              (let ((owner (window-parameter window 'linum--set-margins))
                    (signaled nil))
                (unless (equal owner (window-margins window))
                  (error "Linum did not record its successful margin setting"))
                (advice-add 'set-window-margins :before
                            #'remacs-1687-fail-margin-setter)
                (unwind-protect
                    (condition-case nil
                        (set-window-margins window 5 0)
                      (error (setq signaled t)))
                  (advice-remove 'set-window-margins
                                 #'remacs-1687-fail-margin-setter))
                (unless signaled
                  (error "The injected failing setter unexpectedly returned"))
                (unless (equal (window-parameter window 'linum--set-margins)
                               owner)
                  (error "A failed setter cleared Linum margin ownership")))
              (set-window-margins window 4 0)
              (unless (null (window-parameter window 'linum--set-margins))
                (error "Same-value external setter kept Linum ownership"))
              (linum-delete-overlays)
              (remacs-1687-check-margins 'same-value-external-setter window 4 0)))
        (when (buffer-live-p buffer)
          (kill-buffer buffer)))))
  ;; An explicit setter for another window must not clear this window's owner;
  ;; a nil WINDOW argument must resolve to and clear only the selected window.
  (let ((buffer (generate-new-buffer " *linum-1687-two-windows*")))
    (unwind-protect
        (save-window-excursion
          (delete-other-windows)
          (let* ((first (selected-window))
                 (second (split-window first nil 'right)))
            (set-window-buffer first buffer)
            (set-window-buffer second buffer)
            (with-current-buffer buffer
              (set-window-margins first 0 1)
              (set-window-margins second 0 2)
              (linum--set-window-margins first 4 1)
              (linum--set-window-margins second 5 2)
              (let ((first-owner (window-parameter first 'linum--set-margins)))
                (set-window-margins second 5 2)
                (unless (equal (window-parameter first 'linum--set-margins)
                               first-owner)
                  (error "Setter for another window cleared Linum ownership"))
                (unless (null (window-parameter second 'linum--set-margins))
                  (error "Explicit external setter kept the target owner")))
              (linum-delete-overlays)
              (remacs-1687-check-margins 'explicit-other-window-first first 0 1)
              (remacs-1687-check-margins 'explicit-other-window-second second 5 2)
              (linum--set-window-margins first 4 1)
              (linum--set-window-margins second 5 2)
              (select-window second)
              (set-window-margins nil 5 2)
              (unless (window-parameter first 'linum--set-margins)
                (error "Nil-window setter cleared the non-selected owner"))
              (unless (null (window-parameter second 'linum--set-margins))
                (error "Nil-window setter did not clear selected owner"))
              (linum-delete-overlays)
              (remacs-1687-check-margins 'nil-window-first first 0 1)
              (remacs-1687-check-margins 'nil-window-selected second 5 2))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer))))
  ;; The Rust wrapper loads this source twice; the advice is idempotent and is
  ;; removed when Linum unloads.
  (linum-unload-function)
  (when (advice-member-p #'linum--forget-margin-ownership
                         'set-window-margins)
    (error "Linum margin advice survived library unload"))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

#[test]
fn linum_margin_ownership_tracks_successful_window_setters_1687() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/linum.el");
    let source = lisp_string(source.to_str().unwrap());
    let code = format!(
        "(progn (setq noninteractive t) (load {0} nil t t) (load {0} nil t t) {1})",
        source, CASES
    );
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
}
