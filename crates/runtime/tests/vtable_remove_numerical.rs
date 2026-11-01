use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'vtable)
  (defvar remacs-vtable-getter-calls 0)
  (let ((removed-result
         (with-temp-buffer
           (setq remacs-vtable-getter-calls 0)
           (let* ((removed (vector "text" "drop"))
                  (two (vector 2 "two"))
                  (ten (vector 10 "ten"))
                  (table (make-vtable
                          :columns ["Value" (:name "Label" :align right)]
                          :objects (list removed two ten)
                          :sort-by '((0 . ascend))
                          :getter (lambda (object index table)
                                    (setq remacs-vtable-getter-calls
                                          (1+ remacs-vtable-getter-calls))
                                    (aref object index))))
                  (value-column (car (vtable-columns table)))
                  (label-column (cadr (vtable-columns table)))
                  (before-numerical
                   (vtable-column--numerical value-column))
                  (before-align (vtable-column-align value-column))
                  (calls-before-removal remacs-vtable-getter-calls))
             (setq buffer-read-only t)
             (let ((remove-result (vtable-remove-object table removed)))
               (let ((calls-after-removal remacs-vtable-getter-calls)
                     (after-numerical (vtable-column--numerical value-column))
                     (after-align (vtable-column-align value-column)))
                 (vtable-revert table)
                 (list before-numerical before-align
                       after-numerical after-align
                       (vtable-column-align label-column)
                       (null remove-result)
                       (eq buffer-read-only t)
                       (and (memq two (vtable-objects table))
                            (memq ten (vtable-objects table))
                            (not (memq removed (vtable-objects table))))
                       (and (eq value-column (car (vtable-columns table)))
                            (eq label-column (cadr (vtable-columns table))))
                       (equal (vtable-sort-by table) '((0 . ascend)))
                       (= calls-before-removal calls-after-removal)
                       (mapcar (lambda (line) (mapcar #'car (cdr line)))
                               (vtable--cache-lines
                                (vtable--current-cache table)))))))))
        (mixed-result
         (with-temp-buffer
           (setq remacs-vtable-getter-calls 0)
           (let* ((removed (vector "first"))
                  (remaining (vector "second"))
                  (number (vector 2))
                  (table (make-vtable
                          :columns ["Value"]
                          :objects (list removed remaining number)
                          :getter (lambda (object index table)
                                    (setq remacs-vtable-getter-calls
                                          (1+ remacs-vtable-getter-calls))
                                    (aref object index))))
                  (column (car (vtable-columns table)))
                  (calls-before-removal remacs-vtable-getter-calls))
             (vtable-remove-object table removed)
             (list (vtable-column--numerical column)
                   (vtable-column-align column)
                   (= calls-before-removal remacs-vtable-getter-calls)
                   (eq (car (vtable-objects table)) remaining)
                   (eq (cadr (vtable-objects table)) number)))))
        (empty-result
         (with-temp-buffer
           (setq remacs-vtable-getter-calls 0)
           (let* ((only (vector "only"))
                  (table (make-vtable
                          :columns ["Value"] :objects (list only)
                          :getter (lambda (object index table)
                                    (setq remacs-vtable-getter-calls
                                          (1+ remacs-vtable-getter-calls))
                                    (aref object index))))
                  (column (car (vtable-columns table)))
                  (before-numerical (vtable-column--numerical column))
                  (before-align (vtable-column-align column))
                  (calls-before-removal remacs-vtable-getter-calls))
             (vtable-remove-object table only)
             (list before-numerical before-align
                   (vtable-column--numerical column)
                   (vtable-column-align column)
                   (= calls-before-removal remacs-vtable-getter-calls)
                   (and (null (vtable-objects table))
                        (null (vtable--cache-lines
                               (vtable--current-cache table)))))))))
    (list removed-result mixed-result empty-result)))
"#;

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

fn run_with_library(relative_path: &str) -> String {
    let mut interp = Interp::new();
    let library = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join(relative_path);
    eval(
        &mut interp,
        &format!("(load {} nil t)", lisp_string(library.to_str().unwrap())),
    );
    eval(&mut interp, CASES)
}

#[test]
fn vtable_remove_object_recomputes_numeric_column_metadata() {
    let expected = "((nil left t right right t t t t t t ((2 \"two\") (10 \"ten\"))) (nil left t t t) (nil left t right t t))";
    assert_eq!(run_with_library("lisp/vtable.el"), expected);
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), expected);
}
