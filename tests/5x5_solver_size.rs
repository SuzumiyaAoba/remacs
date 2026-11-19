mod common;

#[test]
fn solver_rejects_unsupported_grid_size_before_matrix_generation() {
    assert_eq!(
        common::ev_err(
            r#"(progn
                 (require '5x5)
                 (let ((5x5-grid-size 4))
                   (5x5-solver (5x5-make-new-grid))))"#
        ),
        "error"
    );
}
