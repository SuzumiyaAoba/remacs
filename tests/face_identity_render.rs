use remacs_runtime::frontend::{CellColor, render_grid};
use remacs_runtime::lisp::Interp;

#[test]
fn rendered_faces_follow_symbols_instead_of_diagnostic_names() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let mut i = Interp::new();
            i.eval_str(
                r##"(let ((wide (intern (concat "render674-" (string #x1a0000))))
                         (literal (intern (concat "render674-" (string 92) "x{1a0000}")))
                         (first (make-symbol "render674-uninterned"))
                         (second (make-symbol "render674-uninterned"))
                         (alias (make-symbol "render674-alias")))
                      (dolist (pair (list (cons wide "#ff0000") (cons literal "#0000ff")
                                         (cons first "#00ff00") (cons second "#ffff00")))
                        (make-face (car pair))
                        (set-face-attribute (car pair) nil :foreground (cdr pair)))
                      (put alias 'face-alias wide)
                      (with-current-buffer "*scratch*"
                        (erase-buffer) (insert "abcde\n")
                        (put-text-property 1 2 'face wide)
                        (put-text-property 2 3 'face literal)
                        (put-text-property 3 4 'face first)
                        (put-text-property 4 5 'face second)
                        (put-text-property 5 6 'face alias)
                        (goto-char 1) (setq mark-active nil)))"##,
            )
            .expect("create distinct face symbols");
            let grid = render_grid(&i, 80, 24);
            let colors: Vec<_> = grid.attrs[0][..5].iter().map(|style| style.fg).collect();
            assert_eq!(
                colors,
                vec![
                    Some(CellColor(255, 0, 0)),
                    Some(CellColor(0, 0, 255)),
                    Some(CellColor(0, 255, 0)),
                    Some(CellColor(255, 255, 0)),
                    Some(CellColor(255, 0, 0)),
                ]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
