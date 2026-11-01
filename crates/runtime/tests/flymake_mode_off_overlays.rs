use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'flymake)
  (defun remacs-2263-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-2263-disable-startup ()
    ;; These bindings belong to each test buffer.  In particular,
    ;; flymake-diagnostic-functions is buffer-local in GNU Flymake.
    (setq-local flymake-diagnostic-functions nil
                flymake-start-on-flymake-mode nil
                flymake-start-on-save-buffer nil
                flymake-no-changes-timeout nil))
  (let ((other-buffer (generate-new-buffer " *flymake-2263-other*")))
    (unwind-protect
        (progn
          (with-current-buffer other-buffer (insert "x"))
          (with-temp-buffer
            (remacs-2263-disable-startup)
            (insert "abcdef")
            (let* ((flymake-inside (make-overlay 1 2))
                     (flymake-outside (make-overlay 5 6))
                     (flymake-at-max (make-overlay (point-max) (point-max)))
                     (plain (make-overlay 2 3))
                     (diagnostic-only (make-overlay 3 4))
                     (foreign (make-overlay 1 2 other-buffer)))
                (overlay-put flymake-inside 'flymake-overlay t)
                (overlay-put flymake-outside 'flymake-overlay t)
                (overlay-put flymake-at-max 'flymake-overlay t)
                (overlay-put foreign 'flymake-overlay t)
                (overlay-put plain 'face 'bold)
                (overlay-put diagnostic-only 'flymake-diagnostic 'decoy)
                (let ((on-result (flymake-mode 1)))
                  (remacs-2263-assert
                   (and (eq on-result t)
                        flymake-mode
                        (eq (overlay-buffer flymake-inside) (current-buffer))
                        (eq (overlay-buffer flymake-outside) (current-buffer))
                        (eq (overlay-buffer flymake-at-max) (current-buffer)))
                   "mode-on changed its return value or deleted owned overlays"))
                (save-restriction
                  ;; Cleanup must widen to cover a Flymake overlay outside the
                  ;; accessible region and one exactly at point-max.
                  (narrow-to-region 1 3)
                  (let ((off-result (flymake-mode-off)))
                    (remacs-2263-assert
                     (null off-result)
                     "mode-off did not return the disabled mode state"))
                  (remacs-2263-assert (not flymake-mode)
                                      "flymake-mode-off left mode enabled")
                  (remacs-2263-assert
                   (and (null (overlay-buffer flymake-inside))
                        (null (overlay-buffer flymake-outside))
                        (null (overlay-buffer flymake-at-max)))
                   "Flymake-owned overlays remained attached after mode-off")
                  (remacs-2263-assert
                   (and (eq (overlay-buffer plain) (current-buffer))
                        (eq (overlay-get plain 'face) 'bold)
                        (eq (overlay-buffer diagnostic-only) (current-buffer))
                        (eq (overlay-get diagnostic-only 'flymake-diagnostic) 'decoy)
                        (eq (overlay-buffer foreign) other-buffer))
                   "mode-off deleted or changed non-owned/foreign overlays"))))
            ;; GNU's overlays-in(point-min, point-max) includes an empty-buffer
            ;; zero-width overlay at point-min == point-max.
            (with-temp-buffer
              (remacs-2263-disable-startup)
              (let ((empty (make-overlay (point-min) (point-max))))
                (overlay-put empty 'flymake-overlay t)
                (remacs-2263-assert (eq (flymake-mode 1) t)
                                    "empty-buffer mode-on return value changed")
                (flymake-mode-off)
                (remacs-2263-assert
                 (null (overlay-buffer empty))
                 "empty-buffer point-min overlay was not cleaned up"))))
        (when (buffer-live-p other-buffer)
          (kill-buffer other-buffer))))
  t)"##;

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
fn flymake_mode_off_deletes_only_flymake_overlays() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    let path = root.join("lisp/flymake.el");
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
    );
    assert_eq!(eval(&mut interp, CASES), "t");
}
