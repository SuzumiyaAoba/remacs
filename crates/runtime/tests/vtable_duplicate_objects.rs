use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'vtable)
  (defvar remacs-vtable-crossreview-getter-calls 0)
  (defun remacs-vtable-crossreview-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-vtable-crossreview-row-objects ()
    (let (objects)
      (save-excursion
        (goto-char (point-min))
        (while (< (point) (point-max))
          (let ((object (get-text-property (line-end-position)
                                           'vtable-object)))
            (when object (push object objects)))
          (forward-line 1)))
      (nreverse objects)))
  (defun remacs-vtable-crossreview-count-eq (object objects)
    (let ((count 0))
      (dolist (candidate objects)
        (when (eq candidate object)
          (setq count (1+ count))))
      count))
  (defun remacs-vtable-crossreview-remove-case
      (columns divider sort-by check-order)
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((same (vector "same" "group"))
             (equal-but-distinct (vector "same" "group"))
             (table
              (make-vtable
               :columns columns
               :objects (list same same equal-but-distinct same)
               :sort-by sort-by
               :divider divider
               :use-header-line nil
               :getter
               (lambda (object index table)
                 (setq remacs-vtable-crossreview-getter-calls
                       (1+ remacs-vtable-crossreview-getter-calls))
                 (aref object index))))
             (column (car (vtable-columns table)))
             (columns-object (vtable-columns table))
             (sort-before (vtable-sort-by table))
             (metadata-before
              (list (vtable-column--numerical column)
                    (vtable-column-align column)
                    (vtable-column--aligned column)))
             (cache (vtable--current-cache table))
             (calls-before remacs-vtable-crossreview-getter-calls)
             (rows-before (remacs-vtable-crossreview-row-objects)))
        (setq buffer-read-only t)
        (remacs-vtable-crossreview-assert
         (= (remacs-vtable-crossreview-count-eq same rows-before) 3)
         "all three EQ duplicate rows were not displayed before removal")
        (remacs-vtable-crossreview-assert
         (= (remacs-vtable-crossreview-count-eq equal-but-distinct rows-before) 1)
         "the equal-but-not-EQ row was not displayed before removal")
        (when check-order
          (remacs-vtable-crossreview-assert
           (equal rows-before (list same same equal-but-distinct same))
           "adjacent property-run row order changed before removal"))
        (let ((remove-result (vtable-remove-object table same)))
          (let* ((objects (vtable-objects table))
                 (cache-lines (vtable--cache-lines cache))
                 (rows-after (remacs-vtable-crossreview-row-objects))
                 (metadata-after
                  (list (vtable-column--numerical column)
                        (vtable-column-align column)
                        (vtable-column--aligned column)))
                 (columns-after (vtable-columns table)))
            (remacs-vtable-crossreview-assert
             (null remove-result) "remove return value changed")
            (remacs-vtable-crossreview-assert
             (and (= (length objects) 1) (eq (car objects) equal-but-distinct))
             "vtable-objects did not remove all and only EQ duplicates")
            (remacs-vtable-crossreview-assert
             (and (= (length cache-lines) 1)
                  (eq (caar cache-lines) equal-but-distinct))
             "cache lines did not remove all and only EQ duplicates")
            (remacs-vtable-crossreview-assert
             (equal rows-after (list equal-but-distinct))
             "display did not remove every duplicate row")
            (remacs-vtable-crossreview-assert
             (and (equal same equal-but-distinct)
                  (not (eq same equal-but-distinct)))
             "fixture did not use equal but distinct objects")
            (remacs-vtable-crossreview-assert
             (eq columns-object columns-after)
             "column list identity changed")
            (remacs-vtable-crossreview-assert
             (equal sort-before (vtable-sort-by table))
             "sort settings changed")
            (remacs-vtable-crossreview-assert
             (equal metadata-before metadata-after)
             "column numerical/alignment metadata changed")
            (remacs-vtable-crossreview-assert
             (eq buffer-read-only t) "buffer read-only state changed")
            (remacs-vtable-crossreview-assert
             (= calls-before remacs-vtable-crossreview-getter-calls)
             "removal re-ran user getters")
            t)))))
  ;; One-column rows let adjacent identical object properties coalesce.
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-remove-case ["Name"] nil nil t)
   "adjacent/nonadjacent duplicate removal failed")
  ;; With a divider, vtable-object begins inside the line at its last column.
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-remove-case
    ["Name" "Group"] "|" '((0 . ascend)) nil)
   "divider/multiple-column duplicate removal failed")
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
fn vtable_remove_object_handles_duplicate_identity_and_divider_rows() {
    assert_eq!(run_with_library("lisp/vtable.el"), "t");
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), "t");
}
