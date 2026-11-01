use remacs_runtime::{
    editor::{FrameRef, reflow_frame_windows},
    frontend::{CellColor, render_frame_grid_with_input_cached},
    lisp::{Interp, Value},
};

fn check(i: &Interp, frames: &[FrameRef]) {
    let selected_frame = i.selected_frame.as_ref().unwrap().clone();
    let active_window = selected_frame.borrow().selected.borrow().id;
    let current = i.current_buffer;
    let mut active_lines = 0;
    for frame in frames {
        let grid = render_frame_grid_with_input_cached(i, frame, 80, 16, None);
        assert_eq!(grid.mode_lines.len(), 2);
        for line in &grid.mode_lines {
            let active = line.window_id == active_window;
            active_lines += usize::from(active);
            let expected = if active {
                CellColor(255, 0, 0)
            } else {
                CellColor(0, 0, 255)
            };
            for column in line.left..line.left + line.width {
                assert_eq!(
                    grid.attrs[line.row][column].fg,
                    Some(expected),
                    "frame {} window {} column {column}",
                    frame.borrow().id,
                    line.window_id
                );
                assert_eq!(grid.attrs[line.row][column].bg, Some(CellColor(0, 0, 0)));
            }
        }
        assert!(std::rc::Rc::ptr_eq(
            i.selected_frame.as_ref().unwrap(),
            &selected_frame
        ));
        assert_eq!(i.current_buffer, current);
    }
    assert_eq!(active_lines, 1);
}

#[test]
fn only_the_globally_selected_window_uses_the_active_mode_line_face() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 80 16) (set-window-fringes nil 0 0)
        (setq-local mode-line-format " Face " header-line-format nil tab-line-format nil)
        (setq qa-face-frame-a (selected-frame) qa-face-a-right (split-window-right 40)
              qa-face-frame-b (make-frame '((window-system . ns) (width . 80) (height . 16)
                (menu-bar-lines . 0) (tool-bar-lines . 0) (tab-bar-lines . 0))))
        (select-frame qa-face-frame-b)
        (switch-to-buffer (get-buffer-create "*mode-face-b*"))
        (setq-local mode-line-format " Face " header-line-format nil tab-line-format nil)
        (setq qa-face-b-right (split-window-right 40))
        (dolist (frame (list qa-face-frame-a qa-face-frame-b))
          (set-face-attribute 'mode-line-active frame :foreground "red" :background "black" :inverse-video nil)
          (set-face-attribute 'mode-line-inactive frame :foreground "blue" :background "black" :inverse-video nil))
        (select-frame qa-face-frame-a))"#).unwrap();
    let a = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(b) = i.eval_str("qa-face-frame-b").unwrap() else {
        panic!("frame B")
    };
    for frame in [&a, &b] {
        reflow_frame_windows(frame, 80, 16);
    }
    let frames = [a, b];
    check(&i, &frames);
    for action in [
        "(select-window qa-face-a-right)",
        "(select-frame qa-face-frame-b)",
        "(select-window qa-face-b-right)",
        "(select-frame qa-face-frame-a)",
    ] {
        i.eval_str(action).unwrap();
        check(&i, &frames);
    }
}
