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
    // GNU: the root window of a frame cannot be resized.
    assert_eq!(
        ev("(condition-case e (window-resize (selected-window) -4 nil)
             (error (cadr e)))"),
        "\"Cannot resize the root window of a frame\""
    );
    // GNU -Q --batch: split halves 24→12, resize -4 → window-height 8.
    assert_eq!(
        ev("(progn (split-window)
                  (window-resize (selected-window) -4 nil)
                  (window-height))"),
        "8"
    );
    // GNU -Q --batch: hsplit 80→40, resize -10 horizontal → width 29.
    assert_eq!(
        ev("(progn (split-window (selected-window) nil t)
                  (window-resize (selected-window) -10 t)
                  (window-width))"),
        "29"
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
fn get_buffer_window_list() {
    // GNU 31.1 batch results: minibuffer window only counted when
    // MINIBUF is t (or nil while the minibuffer is active); 'nomini
    // never counts it.  Selected window comes first.
    assert_eq!(
        ev("(list (length (get-buffer-window-list (current-buffer)))
                 (length (get-buffer-window-list (current-buffer) 'nomini t t))
                 (length (get-buffer-window-list \" *Minibuf-0*\"))
                 (length (get-buffer-window-list \" *Minibuf-0*\" t))
                 (length (get-buffer-window-list \" *Minibuf-0*\" 'nomini))
                 (length (get-buffer-window-list nil 'nomini (selected-frame)))
                 (eq (car (get-buffer-window-list nil nil nil t))
                     (selected-window)))"),
        "(1 1 0 1 0 1 t)"
    );
    // `window-normalize-buffer' errors on dead/missing buffers.
    assert_eq!(
        ev("(condition-case e (progn (get-buffer-window-list \"no-such-xyz\")
                                    :noerr)
               (error (car e)))"),
        "error"
    );
    assert_eq!(
        ev("(let ((b (get-buffer-create \"gbwl-dead\")) v)
              (setq v b) (kill-buffer b)
              (condition-case e (progn (get-buffer-window-list v) :noerr)
                  (error (car e))))"),
        "error"
    );
    // INDIRECT: a window showing an indirect buffer counts for its
    // base buffer's window list (and vice versa).
    assert_eq!(
        ev("(let ((base (current-buffer))
                 (ind (make-indirect-buffer (current-buffer) \"gbwl-ind\")))
              (set-window-buffer (selected-window) ind)
              (prog1
                  (list (eq (car (get-buffer-window-list
                                    base 'nomini t t))
                            (selected-window))
                        (eq (car (get-buffer-window-list
                                    ind 'nomini t t))
                            (selected-window)))
                (set-window-buffer (selected-window) base)))"),
        "(t t)"
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
