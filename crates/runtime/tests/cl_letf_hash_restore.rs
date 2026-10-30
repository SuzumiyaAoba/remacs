use remacs_runtime::lisp::Interp;

#[test]
fn cl_letf_restores_absent_gethash_keys() {
    let mut interp = Interp::new();
    let result = interp
        .eval_str(
            r#"
            (progn
              (require 'cl-lib)
              (list
               (let ((table (make-hash-table)))
                 (cl-letf (((gethash 'temporary table) 42)) nil)
                 (list (gethash 'temporary table 'missing)
                       (hash-table-count table)))
               (let ((table (make-hash-table))
                     (key-calls 0)
                     (table-calls 0)
                     (default-calls 0))
                 (cl-letf (((gethash
                             (progn (setq key-calls (1+ key-calls)) 'temporary)
                             (progn (setq table-calls (1+ table-calls)) table)
                             (progn (setq default-calls (1+ default-calls))
                                    'fallback))
                            42))
                   nil)
                 (list key-calls table-calls default-calls
                       (gethash 'temporary table 'missing)
                       (hash-table-count table)))
               (let ((table (make-hash-table)))
                 (puthash 'present nil table)
                 (cl-letf (((gethash 'present table) 42)) nil)
                 (list (gethash 'present table 'missing)
                       (hash-table-count table)))
               (let ((table (make-hash-table)))
                 (list (catch 'done
                         (cl-letf (((gethash 'temporary table) 42))
                           (throw 'done 'done)))
                       (gethash 'temporary table 'missing)
                       (hash-table-count table)))))
            "#,
        )
        .unwrap();
    assert_eq!(
        interp.prin1_to_string(&result),
        "((missing 0) (1 1 1 missing 0) (nil 1) (done missing 0))"
    );
}
