//! Interactive frame-model transitions; GNU batch cannot create extra
//! terminal frames. Reference: frame.c store_frame_param/set_term_frame_name.
mod common;

use common::{interp, on_big_stack};

#[test]
fn names_and_minibuffers_across_frames() {
    on_big_stack(|| {
        let (mut i, _) = interp();
        i.noninteractive = false;
        let source = r#"
(let* ((first (selected-frame)) (second (make-frame)))
  (unless (equal (list (frame-parameter first 'name) (frame-parameter second 'name))
                 '("F1" "F2"))
    (error "Initial automatic names"))
  (let ((failure (condition-case error
                     (set-frame-parameter second 'name "F1")
                   (error error))))
    (unless (and (eq (car failure) 'error)
                 (string-prefix-p "Frame of the form F<num>" (cadr failure)))
      (error "Duplicate automatic name accepted")))
  (unless (equal (frame-parameter second 'name) "F2")
    (error "Failed name change changed reported name"))
  (let ((failure (condition-case error
                     (set-frame-parameter second 'minibuffer (minibuffer-window first))
                   (error error))))
    (unless (and (eq (car failure) 'error)
                 (string-match-p "own minibuffer" (cadr failure)))
      (error "Own minibuffer replaced")))
  (set-frame-parameter second 'minibuffer (minibuffer-window second))
  (unless (eq (frame-parameter second 'minibuffer) t)
    (error "Own minibuffer not normalized"))
  (let ((name (copy-sequence "X3")))
    (set-frame-parameter first 'name name)
    (aset name 0 ?F))
  (set-frame-parameter second 'name "User")
  (set-frame-parameter second 'name nil)
  (unless (equal (frame-parameter second 'name) "F4")
    (error "Automatic name did not skip occupied name"))
  (delete-frame second)
  (when (or (frame-parameter second 'name) (frame-parameters second))
    (error "Dead frame reported parameters"))
  (let ((third (make-frame)))
    (unless (equal (frame-parameter third 'name) "F5")
      (error "Automatic name sequence was reused"))
    (delete-frame third))
  'ok)
"#;
        let result = i
            .eval_str(source)
            .unwrap_or_else(|_| panic!("frame transitions failed"));
        assert_eq!(i.prin1_to_string(&result), "ok");
    });
}
