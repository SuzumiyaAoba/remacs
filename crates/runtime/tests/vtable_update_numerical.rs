use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"
(progn
  (require 'vtable)
  (defvar remacs-vtable-getter-calls 0)
  (let ((updated-result
         (with-temp-buffer
           (setq remacs-vtable-getter-calls 0)
           (let* ((old (vector "text" "old"))
                  (two (vector 2 "two"))
                  (ten (vector 10 "ten"))
                  (updated (vector 3 "three"))
                  (non-numeric (vector "again" "three"))
                  (table (make-vtable
                          :columns ["Value" (:name "Label" :align right)]
                          :objects (list old two ten)
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
                  (calls-before-update remacs-vtable-getter-calls))
             (setq buffer-read-only t)
             (vtable-update-object table updated old)
             (let ((calls-after-update remacs-vtable-getter-calls)
                   (after-numerical
                    (vtable-column--numerical value-column))
                   (after-align (vtable-column-align value-column)))
               (vtable-revert table)
               (let ((numeric-order
                      (mapcar (lambda (line) (mapcar #'car (cdr line)))
                              (vtable--cache-lines
                               (vtable--current-cache table))))
                     (replaced-and-retained
                      (and (eq (car (vtable-objects table)) updated)
                           (memq two (vtable-objects table))
                           (memq ten (vtable-objects table))
                           (not (memq old (vtable-objects table)))))
                     (calls-before-reverse remacs-vtable-getter-calls))
                 (vtable-update-object table non-numeric updated)
                 (list before-numerical before-align
                       after-numerical after-align
                       (vtable-column-align label-column)
                       (eq buffer-read-only t)
                       replaced-and-retained
                       (and (eq value-column (car (vtable-columns table)))
                            (eq label-column (cadr (vtable-columns table))))
                       (equal (vtable-sort-by table) '((0 . ascend)))
                       numeric-order
                       (= calls-after-update (+ calls-before-update 2))
                       (list (vtable-column--numerical value-column)
                             (vtable-column-align value-column))
                       (= remacs-vtable-getter-calls
                          (+ calls-before-reverse 2))))))))
        (mixed-result
         (with-temp-buffer
           (setq remacs-vtable-getter-calls 0)
           (let* ((old (vector "old"))
                  (remaining (vector "remaining"))
                  (two (vector 2))
                  (updated (vector 3))
                  (table (make-vtable
                          :columns ["Value"]
                          :objects (list old remaining two)
                          :getter (lambda (object index table)
                                    (setq remacs-vtable-getter-calls
                                          (1+ remacs-vtable-getter-calls))
                                    (aref object index))))
                  (column (car (vtable-columns table)))
                  (calls-before-update remacs-vtable-getter-calls))
             (vtable-update-object table updated old)
             (list (vtable-column--numerical column)
                   (vtable-column-align column)
                   (= remacs-vtable-getter-calls
                      (+ calls-before-update 1))
                   (and (eq (car (vtable-objects table)) updated)
                        (eq (cadr (vtable-objects table)) remaining)
                        (eq (caddr (vtable-objects table)) two))))))
        (insert-result
         (with-temp-buffer
           (setq remacs-vtable-getter-calls 0)
           (let* ((number (vector 2))
                  (text (vector "text"))
                  (table (make-vtable
                          :columns ["Value"] :objects (list number)
                          :use-header-line nil
                          :getter (lambda (object index table)
                                    (setq remacs-vtable-getter-calls
                                          (1+ remacs-vtable-getter-calls))
                                    (aref object index))))
                  (column (car (vtable-columns table))))
             (let ((remove-result (vtable-remove-object table number)))
               (let ((before-numerical (vtable-column--numerical column))
                     (before-align (vtable-column-align column))
                     (calls-before-insert remacs-vtable-getter-calls))
                 (vtable-insert-object table text)
                 (list before-numerical before-align
                       (vtable-column--numerical column)
                       (vtable-column-align column)
                       (null remove-result)
                       (eq (car (vtable-objects table)) text)
                       (= remacs-vtable-getter-calls
                          (+ calls-before-insert 2)))))))))
    (list updated-result mixed-result insert-result)))
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
fn vtable_update_object_recomputes_numeric_column_metadata_after_type_changes() {
    let expected = "((nil left t right right t t t t ((2 \"two\") (3 \"three\") (10 \"ten\")) t (nil left) t) (nil left t t) (t right nil left t t t))";
    assert_eq!(run_with_library("lisp/vtable.el"), expected);
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), expected);
}
