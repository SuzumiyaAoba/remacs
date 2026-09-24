//! Tests for window/frame primitives (src/editor/{mod,winxtra}.rs).

mod common;
use common::ev;

#[test]
fn window_basics() {
    assert_eq!(ev("(windowp (selected-window))"), "t");
    assert_eq!(ev("(windowp nil)"), "nil");
    assert_eq!(ev("(window-live-p (selected-window))"), "t");
    assert_eq!(ev("(window-valid-p (selected-window))"), "t");
    assert_eq!(ev("(window-valid-p 5)"), "nil");
    assert_eq!(
        ev("(buffer-name (window-buffer (selected-window)))"),
        "\"*scratch*\""
    );
}

#[test]
fn frame_basics() {
    assert_eq!(ev("(framep (selected-frame))"), "t");
    assert_eq!(ev("(frame-live-p (selected-frame))"), "t");
    assert_eq!(ev("(frame-width)"), "80");
    // Emacs -Q batch frame: 25 rows (24 window + echo area).
    assert_eq!(ev("(frame-height)"), "25");
    assert_eq!(ev("(length (frame-list))"), "1");
    assert_eq!(ev("(frame-char-width)"), "1");
    assert_eq!(ev("(frame-char-height)"), "1");
}

#[test]
fn window_edges() {
    assert_eq!(ev("(window-edges)"), "(0 0 80 24)");
    assert_eq!(ev("(window-pixel-edges)"), "(0 0 80 24)");
    assert_eq!(ev("(window-inside-pixel-edges)"), "(0 0 80 24)");
    // Body excludes the mode line.
    assert_eq!(ev("(window-body-pixel-edges)"), "(0 0 80 23)");
}

#[test]
fn window_sizes() {
    assert_eq!(ev("(window-height)"), "24");
    assert_eq!(ev("(window-width)"), "80");
    assert_eq!(ev("(window-total-height)"), "24");
    assert_eq!(ev("(window-text-height)"), "24");
    assert_eq!(ev("(window-pixel-width)"), "80");
    assert_eq!(ev("(window-pixel-height)"), "24");
}

#[test]
fn split_and_delete() {
    assert_eq!(
        ev(
            "(let ((w2 (split-window-internal (selected-window) 10 'right nil)))
              (list (length (window-list-1))
                    (window-edges (selected-window))
                    (window-edges w2)))"
        ),
        "(2 (0 0 10 24) (10 0 80 24))"
    );
    assert_eq!(
        ev(
            "(let ((w2 (split-window-internal (selected-window) 8 'below nil)))
              (list (window-edges (selected-window))
                    (window-edges w2)))"
        ),
        "((0 0 80 8) (0 8 80 24))"
    );
    // Deleting the new window leaves the original.
    assert_eq!(
        ev(
            "(let ((w2 (split-window-internal (selected-window) 10 'right nil)))
              (delete-window-internal w2)
              (list (length (window-list-1))
                    (window-live-p w2)))"
        ),
        "(1 nil)"
    );
}

#[test]
fn window_resize() {
    assert_eq!(
        ev("(progn (window-resize (selected-window) -4 nil)
                  (window-height))"),
        "20"
    );
    assert_eq!(
        ev("(progn (window-resize (selected-window) -10 t)
                  (window-width))"),
        "70"
    );
}

#[test]
fn window_navigation() {
    assert_eq!(
        ev("(let ((w (selected-window))
                (w2 (split-window-internal (selected-window) 10 'right nil)))
              (list (eq (window-next-sibling w) w2)
                    (eq (window-prev-sibling w2) w)))"),
        "(t t)"
    );
    assert_eq!(ev("(eq (frame-root-window) (car (window-list-1)))"), "t");
    assert_eq!(ev("(eq (frame-selected-window) (selected-window))"), "t");
}

#[test]
fn minibuffer_window() {
    assert_eq!(ev("(windowp (minibuffer-window))"), "t");
    assert_eq!(
        ev("(buffer-name (window-buffer (minibuffer-window)))"),
        "\" *Minibuf-0*\""
    );
}

#[test]
fn frame_params() {
    assert_eq!(
        ev("(progn (set-frame-height (selected-frame) 40)
                  (frame-height))"),
        "40"
    );
    assert_eq!(
        ev("(progn (set-frame-width (selected-frame) 100)
                  (frame-width))"),
        "100"
    );
    assert_eq!(
        ev("(progn (set-frame-size (selected-frame) 60 30)
                  (list (frame-width) (frame-height)))"),
        "(60 30)"
    );
}

#[test]
fn window_params_and_margins() {
    // GNU batch: margins is the cons (nil . nil).
    assert_eq!(ev("(window-margins)"), "(nil)");
    assert_eq!(ev("(window-vscroll)"), "0");
    assert_eq!(ev("(window-has-parameters)"), "nil");
    assert_eq!(ev("(window-scroll-bars)"), "(nil 0 t nil 0 t nil)");
}

#[test]
fn posn_at_point() {
    // posn-at-point returns a posn list; just check it's a list.
    assert_eq!(ev("(consp (posn-at-point 1))"), "t");
}

#[test]
fn scroll_lr() {
    assert_eq!(ev("(progn (scroll-left 5) (window-hscroll))"), "5");
    assert_eq!(
        ev("(progn (scroll-left 5) (scroll-right 3) (window-hscroll))"),
        "2"
    );
}
