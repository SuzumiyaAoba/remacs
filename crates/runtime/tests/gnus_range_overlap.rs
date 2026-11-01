use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-2652-check (label expected actual)
    (unless (equal actual expected)
      (error "Gnus range nconcat #2652 %S: expected %S, got %S"
             label expected actual)))
  ;; The public operation is destructive, so ordinary fixtures are copied.
  (defun remacs-2652-call (&rest ranges)
    (apply #'gnus-range-nconcat (mapcar #'copy-tree ranges)))
  (defun remacs-2652-expand (range)
    (let (numbers)
      (range-map (lambda (number) (push number numbers)) range)
      (nreverse numbers)))
  ;; Merge all overlaps, including overlaps already present in the first
  ;; range and overlap components after an initially disjoint gap.
  (remacs-2652-check 'overlapping-pairs '(1 . 8)
                     (remacs-2652-call '(1 . 5) '(4 . 8)))
  (remacs-2652-check 'contained-second-range '(1 . 8)
                     (remacs-2652-call '(1 . 8) '(3 . 5)))
  (remacs-2652-check 'same-start-extends-end '(1 . 9)
                     (remacs-2652-call '(1 . 5) '(1 . 9)))
  (remacs-2652-check 'duplicate-range '(1 . 5)
                     (remacs-2652-call '(1 . 5) '(1 . 5)))
  (remacs-2652-check 'initial-range-components-coalesce '(1 . 8)
                     (remacs-2652-call '((1 . 5) (4 . 8))))
  (remacs-2652-check 'overlap-after-gap-merges-every-component
                     '((1 . 2) (5 . 12))
                     (remacs-2652-call '(1 . 2) '((5 . 9) (7 . 12))))
  (remacs-2652-check 'overlap-prefix-merges-every-component '(1 . 13)
                     (remacs-2652-call
                      '(1 . 10) '((4 . 5) (7 . 8) (11 . 13))))
  (remacs-2652-check 'multiple-overlap-chain '(1 . 10)
                     (remacs-2652-call '(1 . 3) '(3 . 5) '(5 . 7) '(6 . 10)))
  ;; Preserve adjacent, disjoint, and compact integer-singleton forms.
  (remacs-2652-check 'adjacent-pairs '(1 . 8)
                     (remacs-2652-call '(1 . 3) '(4 . 8)))
  (remacs-2652-check 'disjoint-pairs '((1 . 3) (6 . 8))
                     (remacs-2652-call '(1 . 3) '(6 . 8)))
  (remacs-2652-check 'proper-singleton-adjacent '(3 . 4)
                     (remacs-2652-call '(3) '(4)))
  (remacs-2652-check 'proper-singleton-duplicate '(3)
                     (remacs-2652-call '(3) '(3)))
  (remacs-2652-check 'proper-singleton-disjoint '(3 8)
                     (remacs-2652-call '(3) '(8)))
  (remacs-2652-check 'singleton-overlaps-pair '(3 . 8)
                     (remacs-2652-call '(3) '(4 . 8)))
  ;; Empty arguments and proper-list range formats remain supported.
  (remacs-2652-check 'no-arguments nil (gnus-range-nconcat))
  (remacs-2652-check 'all-empty-arguments nil (gnus-range-nconcat nil nil nil))
  (remacs-2652-check 'empty-arguments-between-ranges '((1 . 2) (5 . 6))
                     (remacs-2652-call nil '(1 . 2) nil '(5 . 6)))
  (remacs-2652-check 'proper-list-range-input '(1 . 8)
                     (remacs-2652-call '((1 . 5)) '((4 . 8))))
  ;; Verify the uncompressed union only on a small range.
  (remacs-2652-check 'small-uncompressed-union '(1 2 3 4 5 6 7 8)
                     (remacs-2652-expand
                      (remacs-2652-call '(1 . 5) '(4 . 8))))
  ;; A large range is compared structurally; this does not expand its members.
  (let* ((large (ash 1 100))
         (actual (remacs-2652-call
                  (cons large (+ large 5))
                  (cons (+ large 4) (+ large 10)))))
    (remacs-2652-check 'bignum-overlap (cons large (+ large 10)) actual))
  (let* ((large (ash 1 100))
         (actual (remacs-2652-call (list large) (list (1+ large)))))
    (remacs-2652-check 'bignum-adjacency (cons large (1+ large)) actual))
  ;; nconcat widens the first range and reuses list tails for disjoint input.
  (let* ((left (cons 1 5))
         (right (cons 4 8))
         (actual (gnus-range-nconcat left right)))
    (unless (eq actual left)
      (error "Overlapping nconcat did not retain the first range object"))
    (unless (= (cdr left) 8)
      (error "Overlapping nconcat did not widen the first range object")))
  (let* ((left (list (cons 1 2)))
         (right (list (cons 6 8)))
         (actual (gnus-range-nconcat left right)))
    (unless (and (eq actual left) (eq (cdr left) right))
      (error "Disjoint nconcat did not link the input list tails")))
  t)"###;

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

#[test]
fn gnus_range_nconcat_merges_sorted_overlapping_ranges_2652() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let range = root.join("lisp/range.el");
    let gnus_range = root.join("lisp/gnus-range.el");
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) (load {} nil t t) {})",
        lisp_string(range.to_str().unwrap()),
        lisp_string(gnus_range.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
}
