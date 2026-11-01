use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'cl-lib)
  (require 'flymake)
  (defun remacs-2249-assert (condition message)
    (unless condition (error "%s" message)))
  ;; Exact issue reproduction: the diagnostic exists only on the overlay.
  (with-temp-buffer
    (insert "abc")
    (let* ((diag (flymake-make-diagnostic
                  (current-buffer) 1 2 :error "bad"))
           (overlay (make-overlay 1 2)))
      (overlay-put overlay 'flymake-diagnostic diag)
      (let ((found (flymake-diagnostics)))
        (remacs-2249-assert
         (and (= (length found) 1)
              (eq (car found) diag)
              (equal (mapcar #'flymake-diagnostic-message found) '("bad")))
         "overlay-backed diagnostic was omitted or reconstructed"))))
  ;; Overlay queries follow overlays-at/overlays-in boundaries, preserve
  ;; traversal order, ignore plain overlays, and do not deduplicate identity.
  (with-temp-buffer
    (insert "abcdef")
    (let* ((left (flymake-make-diagnostic
                  (current-buffer) 2 4 :error "left"))
           (right (flymake-make-diagnostic
                   (current-buffer) 4 6 :warning "right"))
           (left-1 (make-overlay 2 4))
           (left-2 (make-overlay 2 4))
           (right-overlay (make-overlay 4 6))
           (plain (make-overlay 1 2)))
      (overlay-put left-1 'flymake-diagnostic left)
      (overlay-put left-2 'flymake-diagnostic left)
      (overlay-put right-overlay 'flymake-diagnostic right)
      (let ((all (flymake-diagnostics)))
        (remacs-2249-assert
         (and (= (length all) 3)
              (eq (nth 0 all) left)
              (eq (nth 1 all) left)
              (eq (nth 2 all) right))
         "overlay result order, identity, or duplicate semantics changed"))
      (remacs-2249-assert
       (equal (mapcar #'flymake-diagnostic-message
                      (flymake-diagnostics 3 4))
              '("left" "left"))
       "overlays-in region boundaries changed")
      (remacs-2249-assert
       (equal (mapcar #'flymake-diagnostic-message
                      (flymake-diagnostics 4 5))
              '("right"))
       "an overlay ending at BEG or starting inside the region was mishandled")
      (remacs-2249-assert
       (equal (mapcar #'flymake-diagnostic-message
                      (flymake-diagnostics 3))
              '("left" "left"))
       "BEG-only query did not use overlays at BEG")
      (remacs-2249-assert
       (equal (mapcar #'flymake-diagnostic-message
                      (flymake-diagnostics 4))
              '("right"))
       "point query included the previous overlay at its end boundary")))
  ;; GNU widens when selecting overlay diagnostics, even when the caller has
  ;; narrowed the current buffer.
  (with-temp-buffer
    (insert "abcde")
    (let* ((diag (flymake-make-diagnostic
                  (current-buffer) 5 6 :error "outside"))
           (overlay (make-overlay 5 6)))
      (overlay-put overlay 'flymake-diagnostic diag)
      (save-restriction
        (narrow-to-region 1 4)
        (remacs-2249-assert
         (eq (car (flymake-diagnostics)) diag)
         "overlay query did not widen the current buffer"))))
  ;; Keep the original region/point query coverage, represented only by real
  ;; GNU diagnostic overlays rather than the private Remacs list.
  (with-temp-buffer
    (insert "abcdef")
    (let* ((first (flymake-make-diagnostic
                   (current-buffer) 1 2 :error "first"))
           (second (flymake-make-diagnostic
                    (current-buffer) 5 6 :warning "second"))
           (first-overlay (make-overlay 1 2))
           (second-overlay (make-overlay 5 6)))
      (overlay-put first-overlay 'flymake-diagnostic first)
      (overlay-put second-overlay 'flymake-diagnostic second)
      (remacs-2249-assert
       (equal (list (length (flymake-diagnostics))
                    (length (flymake-diagnostics 1 2))
                    (mapcar #'flymake-diagnostic-message
                            (flymake-diagnostics 5))
                    (length (flymake-diagnostics 3)))
              '(2 1 ("second") 0))
       "overlay-backed diagnostics region/point behavior changed")))
  ;; Overlay placement is canonical: moving the overlay moves the diagnostic
  ;; query result even though the record retains its original BEG and END.
  (with-temp-buffer
    (insert "abcdef")
    (let* ((diag (flymake-make-diagnostic
                  (current-buffer) 1 2 :error "moved"))
           (unplaced (flymake-make-diagnostic
                      (current-buffer) 3 4 :warning "unplaced"))
           (overlay (make-overlay 1 2)))
      (overlay-put overlay 'flymake-diagnostic diag)
      (move-overlay overlay 5 6)
      (remacs-2249-assert
       (and (equal (mapcar #'flymake-diagnostic-message (flymake-diagnostics))
                   '("moved"))
            (null (flymake-diagnostics 1 2))
            (eq (car (flymake-diagnostics 5 6)) diag)
            (not (memq unplaced (flymake-diagnostics))))
       "diagnostic query did not follow its canonical overlay")))
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
fn flymake_diagnostics_collects_overlay_backed_diagnostics() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    let path = root.join("lisp/flymake.el");
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(path.to_str().unwrap())),
    );
    assert_eq!(eval(&mut interp, CASES), "t");
}
