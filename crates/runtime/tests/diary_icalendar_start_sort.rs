use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'diary-icalendar)
  (defun remacs-test-start-component (uid start)
    (icalendar-make-ast-node
     'icalendar-vtodo nil
     (append
      (list (icalendar-make-property icalendar-uid uid)
            (icalendar-make-property icalendar-dtstamp
              (icalendar-read-date-time "20250101T000000Z")))
      (when start
        (list (icalendar-make-property icalendar-dtstart
                (if (> (length start) 8) (icalendar-read-date-time start)
                  (icalendar-read-date start))))))))
  (let ((a (remacs-test-start-component "missing-a" nil))
        (b (remacs-test-start-component "missing-b" nil)))
    (when (or (diary-icalendar-sort-by-start-ascending a b)
              (diary-icalendar-sort-by-start-ascending b a)
              (diary-icalendar-sort-by-start-ascending a a))
      (error "Missing-start components must not precede each other or themselves"))
    (unless (equal (mapcar #'icalendar-component-uid
                           (sort (list b a) #'diary-icalendar-sort-by-start-ascending))
                   '("missing-b" "missing-a"))
      (error "Missing-start components lost their original order")))
  ;; Compare real nodes against independently declared ranks.  Equal ranks
  ;; cover both absent starts and separate nodes with equal start values.
  (dolist (definitions
           '((("late" "20250102" 1) ("missing-b" nil 2)
              ("tie" "20250101" 0) ("early" "20250101" 0) ("missing-a" nil 2))
             (("late" "20250101T110000" 1) ("missing-b" nil 2)
              ("tie" "20250101T090000" 0) ("early" "20250101T090000" 0)
              ("missing-a" nil 2))))
    (let ((rows (mapcar (lambda (definition)
                         (cons (remacs-test-start-component (car definition) (cadr definition))
                               (nth 2 definition)))
                       definitions)))
      (dolist (left rows)
        (dolist (right rows)
          (unless (eq (not (null (diary-icalendar-sort-by-start-ascending
                                  (car left) (car right))))
                      (< (cdr left) (cdr right)))
            (error "The start comparator violated its strict ordering"))))
      (unless (equal (mapcar #'icalendar-component-uid
                             (sort (mapcar #'car rows)
                                   #'diary-icalendar-sort-by-start-ascending))
                     '("tie" "early" "late" "missing-b" "missing-a"))
        (error "Mixed start sorting was not ascending and stable"))))
  (unless (eq diary-icalendar-import-comparison-function
              #'diary-icalendar-sort-by-start-ascending)
    (error "The tested comparator is not the default diary import comparator"))
  t)
"##;

#[test]
fn missing_start_components_are_equal_in_the_diary_import_order() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
