use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'vtable)
  (defvar remacs-vtable3028-getter-calls 0)
  (defvar remacs-vtable3028-formatter-calls 0)
  (defvar remacs-vtable3028-displayer-calls 0)
  (defun remacs-vtable3028-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-vtable3028-getter (object index _table)
    (setq remacs-vtable3028-getter-calls
          (1+ remacs-vtable3028-getter-calls))
    (elt object index))
  (defun remacs-vtable3028-formatter (value _index _table)
    (setq remacs-vtable3028-formatter-calls
          (1+ remacs-vtable3028-formatter-calls))
    (if (stringp value) value (format "%s" value)))
  (defun remacs-vtable3028-displayer (value _index _width _table)
    (setq remacs-vtable3028-displayer-calls
          (1+ remacs-vtable3028-displayer-calls))
    value)
  (defun remacs-vtable3028-eq-list-p (actual expected)
    (and (= (length actual) (length expected))
         (cl-loop for a in actual for e in expected always (eq a e))))
  (defun remacs-vtable3028-display-rows (table)
    (with-current-buffer (vtable-buffer table)
      (save-excursion
        (unless (vtable-goto-table table)
          (error "Could not find table"))
        (when (and (slot-value table '-has-column-spec)
                   (not (vtable-use-header-line table)))
          (forward-line 1))
        (let (rows)
          (while (< (point) (point-max))
            (let* ((end (line-end-position))
                   (object (get-text-property end 'vtable-object)))
              (when object
                (push (list object
                            (buffer-substring-no-properties
                             (line-beginning-position) end))
                      rows)))
            (forward-line 1))
          (nreverse rows)))))
  (defun remacs-vtable3028-cache-objects (table)
    (mapcar #'car (vtable--cache-lines (vtable--current-cache table))))
  (defun remacs-vtable3028-test-equal-copy-nonhead-update ()
    (with-temp-buffer
      (setq remacs-vtable3028-getter-calls 0
            remacs-vtable3028-formatter-calls 0
            remacs-vtable3028-displayer-calls 0)
      (let* ((a (list "a" 1))
             (old (list "b" "unknown"))
             (c (list "c" 3))
             (old-copy (copy-tree old))
             (replacement (list "b" 2))
             (table (make-vtable
                     :columns ["Name" "Count"]
                     :objects (list a old c)
                     :sort-by '((0 . ascend))
                     :use-header-line nil
                     :getter #'remacs-vtable3028-getter
                     :formatter #'remacs-vtable3028-formatter
                     :displayer #'remacs-vtable3028-displayer))
             (objects-before (vtable-objects table))
             (columns (vtable-columns table))
             (count-column (cadr columns))
             (sort-before (vtable-sort-by table))
             (cache (vtable--current-cache table))
             (cache-lines (vtable--cache-lines cache))
             (target-line (nth 1 cache-lines))
             (cache-table (slot-value table '-cache))
             (cache-key (slot-value table '-current-cache-key))
             (other-key (cons (car cache-key) (1+ (cdr cache-key))))
             (other-cache (copy-tree cache))
             (getter-before remacs-vtable3028-getter-calls)
             (formatter-before remacs-vtable3028-formatter-calls)
             (displayer-before remacs-vtable3028-displayer-calls))
        (remacs-vtable3028-assert
         (and (equal old old-copy) (not (eq old old-copy)))
         "copy-tree fixture must be equal but not EQ")
        (puthash other-key other-cache cache-table)
        (setq buffer-read-only t)
        (let* ((result (vtable-update-object table replacement old-copy))
               (display-rows (remacs-vtable3028-display-rows table))
               (new-cache-lines (vtable--cache-lines cache)))
          (remacs-vtable3028-assert
           (eq result columns)
           "Updating the last non-numerical value changed the return contract")
          (remacs-vtable3028-assert
           (and (eq objects-before (vtable-objects table))
                (remacs-vtable3028-eq-list-p
                 (vtable-objects table) (list a replacement c)))
           "Equal-copy update changed the wrong backing object")
          (remacs-vtable3028-assert
           (and (eq cache (vtable--current-cache table))
                (eq cache-key (slot-value table '-current-cache-key))
                (eq cache-lines new-cache-lines)
                (eq target-line (nth 1 new-cache-lines))
                (remacs-vtable3028-eq-list-p
                 (remacs-vtable3028-cache-objects table)
                 (list a replacement c)))
           "Equal-copy update changed a different cache row")
          (remacs-vtable3028-assert
           (and (remacs-vtable3028-eq-list-p
                 (mapcar #'car display-rows) (list a replacement c))
                (string-match-p "2" (cadr (nth 1 display-rows))))
           "Equal-copy update did not refresh the matching display row")
          (remacs-vtable3028-assert
           (and (not (memq old (vtable-objects table)))
                (not (memq old-copy (vtable-objects table)))
                (eq columns (vtable-columns table)))
           "Equal-copy update retained the old object or replaced columns")
          (remacs-vtable3028-assert
           (and (vtable-column--numerical count-column)
                (eq (vtable-column-align count-column) 'right))
           "Numeric metadata was not recomputed for the updated row")
          (remacs-vtable3028-assert
           (equal sort-before (vtable-sort-by table))
           "Update changed sort settings")
          (remacs-vtable3028-assert
           (and (eq (gethash cache-key cache-table) cache)
                (null (gethash other-key cache-table)))
           "Update did not preserve current cache and invalidate other layouts")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-getter-calls (+ getter-before 2))
           "Equal-copy update getter count was not one call per cell")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-formatter-calls (+ formatter-before 2))
           "Equal-copy update formatter count was not one call per cell")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-displayer-calls (+ displayer-before 2))
           "Equal-copy update displayer count was not one call per cell")
          (remacs-vtable3028-assert
           (eq buffer-read-only t) "Update changed read-only state"))
        t)))
  (defun remacs-vtable3028-test-eq-precedes-earlier-equal-object ()
    (with-temp-buffer
      (setq remacs-vtable3028-getter-calls 0)
      (let* ((first (list "same" "value"))
             (second (copy-tree first))
             (tail (list "tail" "value"))
             (replacement (list "replacement" "value"))
             (table (make-vtable :columns ["Name" "Value"]
                                 :objects (list first second tail)
                                 :use-header-line nil
                                 :getter #'remacs-vtable3028-getter))
             (cache (vtable--current-cache table))
             (cache-lines (vtable--cache-lines cache))
             (second-line (nth 1 cache-lines))
             (getter-before remacs-vtable3028-getter-calls))
        (remacs-vtable3028-assert
         (and (equal first second) (not (eq first second)))
         "EQ-precedence fixture must have distinct equal objects")
        (setq buffer-read-only t)
        (let ((result (vtable-update-object table replacement second)))
          (remacs-vtable3028-assert (null result)
                                    "EQ update return value changed")
          (remacs-vtable3028-assert
           (remacs-vtable3028-eq-list-p
            (vtable-objects table) (list first replacement tail))
           "EQ argument did not select its exact backing object")
          (remacs-vtable3028-assert
           (and (remacs-vtable3028-eq-list-p
                 (remacs-vtable3028-cache-objects table)
                 (list first replacement tail))
                (eq second-line
                    (nth 1 (vtable--cache-lines cache))))
           "EQ argument updated a different equal cache row")
          (remacs-vtable3028-assert
           (remacs-vtable3028-eq-list-p
            (mapcar #'car (remacs-vtable3028-display-rows table))
            (list first replacement tail))
           "EQ argument updated a different equal display row")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-getter-calls (+ getter-before 2))
           "EQ update changed getter call count")
          (remacs-vtable3028-assert
           (eq buffer-read-only t) "EQ update changed read-only state"))
        t)))
  (defun remacs-vtable3028-test-equal-fallback-first-match ()
    (with-temp-buffer
      (let* ((first (list "same"))
             (second (copy-tree first))
             (old-copy (copy-tree second))
             (replacement (list "new"))
             (table (make-vtable :columns ["Name"]
                                 :objects (list first second)
                                 :use-header-line nil)))
        (remacs-vtable3028-assert
         (and (equal first old-copy)
              (not (eq first old-copy))
              (not (eq second old-copy)))
         "equal-fallback fixture must match multiple nonidentical objects")
        (vtable-update-object table replacement old-copy)
        (remacs-vtable3028-assert
         (remacs-vtable3028-eq-list-p
          (vtable-objects table) (list replacement second))
         "Non-EQ equal fallback did not choose the first source match")
        (remacs-vtable3028-assert
         (remacs-vtable3028-eq-list-p
          (remacs-vtable3028-cache-objects table) (list replacement second))
         "Equal fallback updated the wrong cached row")
        (remacs-vtable3028-assert
         (remacs-vtable3028-eq-list-p
          (mapcar #'car (remacs-vtable3028-display-rows table))
          (list replacement second))
         "Equal fallback updated the wrong displayed row")
        t)))
  (defun remacs-vtable3028-test-consecutive-eq-rows ()
    (with-temp-buffer
      (setq remacs-vtable3028-getter-calls 0
            remacs-vtable3028-formatter-calls 0
            remacs-vtable3028-displayer-calls 0)
      (let* ((prefix (vector "prefix"))
             (same (vector "same"))
             (tail (vector "tail"))
             (replacement (vector "replacement"))
             (table (make-vtable :columns ["Name"]
                                 :objects (list prefix same same tail)
                                 :use-header-line nil
                                 :getter #'remacs-vtable3028-getter
                                 :formatter #'remacs-vtable3028-formatter
                                 :displayer #'remacs-vtable3028-displayer))
             (objects (vtable-objects table))
             (cache (vtable--current-cache table))
             (lines (vtable--cache-lines cache))
             (target-line (nth 1 lines))
             (second-duplicate-line (nth 2 lines))
             (key (slot-value table '-current-cache-key))
             (getter-before remacs-vtable3028-getter-calls)
             (formatter-before remacs-vtable3028-formatter-calls)
             (displayer-before remacs-vtable3028-displayer-calls))
        (remacs-vtable3028-assert
         (and (eq (nth 1 objects) same) (eq (nth 2 objects) same)
              (eq (car target-line) same)
              (eq (car second-duplicate-line) same))
         "Repeated-EQ fixture did not contain adjacent duplicate rows")
        (setq buffer-read-only t)
        (let* ((result (vtable-update-object table replacement same))
               (rows (remacs-vtable3028-display-rows table))
               (new-lines (vtable--cache-lines cache)))
          (remacs-vtable3028-assert
           (null result) "Repeated-EQ update return value changed")
          (remacs-vtable3028-assert
           (and (eq objects (vtable-objects table))
                (remacs-vtable3028-eq-list-p
                 (vtable-objects table) (list prefix replacement same tail)))
           "Repeated-EQ update changed the wrong backing occurrence")
          (remacs-vtable3028-assert
           (and (eq lines new-lines)
                (eq target-line (nth 1 new-lines))
                (eq second-duplicate-line (nth 2 new-lines))
                (remacs-vtable3028-eq-list-p
                 (mapcar #'car new-lines)
                 (list prefix replacement same tail)))
           "Repeated-EQ update changed the wrong cache line")
          (remacs-vtable3028-assert
           (remacs-vtable3028-eq-list-p
            (mapcar #'car rows) (list prefix replacement same tail))
           "Repeated-EQ update changed the wrong displayed row")
          (remacs-vtable3028-assert
           (and (string-match-p "replacement" (cadr (nth 1 rows)))
                (string-match-p "same" (cadr (nth 2 rows))))
           "Repeated-EQ update displayed the wrong row contents")
          (remacs-vtable3028-assert
           (eq (slot-value table '-current-cache-key) key)
           "Repeated-EQ update changed the active cache key")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-getter-calls (+ getter-before 1))
           "Repeated-EQ update getter count changed")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-formatter-calls (+ formatter-before 1))
           "Repeated-EQ update formatter count changed")
          (remacs-vtable3028-assert
           (= remacs-vtable3028-displayer-calls (+ displayer-before 1))
           "Repeated-EQ update displayer count changed")
          (remacs-vtable3028-assert
           (eq buffer-read-only t) "Repeated-EQ update changed read-only state"))
        t)))
  (defun remacs-vtable3028-test-adjacent-table-is-not-target ()
    (with-temp-buffer
      (setq remacs-vtable3028-getter-calls 0
            remacs-vtable3028-formatter-calls 0
            remacs-vtable3028-displayer-calls 0)
      (let* ((prefix (vector "prefix"))
             (shared (vector "shared"))
             (replacement (vector "replacement"))
             (table (make-vtable :objects (list prefix shared)
                                 :use-header-line nil
                                 :getter #'remacs-vtable3028-getter
                                 :formatter #'remacs-vtable3028-formatter
                                 :displayer #'remacs-vtable3028-displayer
                                 :insert nil))
             (adjacent (make-vtable :objects (list shared)
                                    :use-header-line nil
                                    :getter #'remacs-vtable3028-getter
                                    :formatter #'remacs-vtable3028-formatter
                                    :displayer #'remacs-vtable3028-displayer
                                    :insert nil)))
        (vtable-insert table)
        (goto-char (point-max))
        (vtable-insert adjacent)
        ;; Remove TABLE's second display row but leave its model/cache entry.
        ;; The adjacent table now occupies the old cache-index position and
        ;; contains the exact same EQ object.
        (save-excursion
          (vtable-goto-table table)
          (forward-line 1)
          (let ((inhibit-read-only t))
            (delete-region (line-beginning-position)
                           (min (point-max) (1+ (line-end-position))))))
        (setq remacs-vtable3028-getter-calls 0
              remacs-vtable3028-formatter-calls 0
              remacs-vtable3028-displayer-calls 0)
        (setq buffer-read-only t)
        (let* ((objects-before (vtable-objects table))
               (cache (vtable--current-cache table))
               (lines (vtable--cache-lines cache))
               (line0 (car lines))
               (line1 (cadr lines))
               (cache-key (slot-value table '-current-cache-key))
               (cache-table (slot-value table '-cache))
               (adjacent-objects (vtable-objects adjacent))
               (adjacent-cache (vtable--current-cache adjacent))
               (adjacent-lines (vtable--cache-lines adjacent-cache))
               (adjacent-rows-before (remacs-vtable3028-display-rows adjacent))
               (text-before (buffer-substring-no-properties
                             (point-min) (point-max)))
               (getter-before remacs-vtable3028-getter-calls)
               (formatter-before remacs-vtable3028-formatter-calls)
               (displayer-before remacs-vtable3028-displayer-calls)
               (signaled
                (let ((buffer-read-only t))
                  (condition-case nil
                      (progn
                        (vtable-update-object table replacement shared)
                        nil)
                    (error t)))))
          (remacs-vtable3028-assert signaled
                                    "Missing row fell through into adjacent table")
          (remacs-vtable3028-assert
           (and (eq objects-before (vtable-objects table))
                (remacs-vtable3028-eq-list-p
                 (vtable-objects table) (list prefix shared)))
           "Adjacent-table failure changed target model")
          (remacs-vtable3028-assert
           (and (eq cache (vtable--current-cache table))
                (eq lines (vtable--cache-lines cache))
                (eq line0 (car lines))
                (eq line1 (cadr lines))
                (eq (car line1) shared))
           "Adjacent-table failure changed target cache")
          (remacs-vtable3028-assert
           (and (eq adjacent-objects (vtable-objects adjacent))
                (eq adjacent-cache (vtable--current-cache adjacent))
                (eq adjacent-lines (vtable--cache-lines adjacent-cache))
                (eq (car (car adjacent-lines)) shared)
                (equal adjacent-rows-before
                       (remacs-vtable3028-display-rows adjacent)))
           "Adjacent-table failure changed the other table")
          (remacs-vtable3028-assert
           (equal text-before (buffer-substring-no-properties
                               (point-min) (point-max)))
           "Adjacent-table failure changed displayed text")
          (remacs-vtable3028-assert
           (and (= getter-before remacs-vtable3028-getter-calls)
                (= formatter-before remacs-vtable3028-formatter-calls)
                (= displayer-before remacs-vtable3028-displayer-calls))
           "Adjacent-table failure called user callbacks")
          (remacs-vtable3028-assert
           (and (eq (gethash cache-key cache-table) cache)
                (eq buffer-read-only t))
           "Adjacent-table failure changed cache key or read-only state"))
        t)))
  (defun remacs-vtable3028-test-notfound-is-atomic ()
    (with-temp-buffer
      (setq remacs-vtable3028-getter-calls 0
            remacs-vtable3028-formatter-calls 0
            remacs-vtable3028-displayer-calls 0)
      (let* ((a (list "a" 1))
             (b (list "b" "unknown"))
             (table (make-vtable
                     :columns ["Name" "Value"]
                     :objects (list a b)
                     :sort-by '((0 . ascend))
                     :use-header-line nil
                     :getter #'remacs-vtable3028-getter
                     :formatter #'remacs-vtable3028-formatter
                     :displayer #'remacs-vtable3028-displayer))
             (objects (vtable-objects table))
             (cache (vtable--current-cache table))
             (lines (vtable--cache-lines cache))
             (line0 (car lines))
             (line1 (cadr lines))
             (key (slot-value table '-current-cache-key))
             (cache-table (slot-value table '-cache))
             (other-key (cons (car key) (1+ (cdr key))))
             (other-cache (copy-tree cache))
             (rows-before (remacs-vtable3028-display-rows table))
             (columns (vtable-columns table))
             (metadata (mapcar (lambda (column)
                                 (list (vtable-column--numerical column)
                                       (vtable-column-align column)))
                               columns))
             (getter-before remacs-vtable3028-getter-calls)
             (formatter-before remacs-vtable3028-formatter-calls)
             (displayer-before remacs-vtable3028-displayer-calls))
        (puthash other-key other-cache cache-table)
        (setq buffer-read-only t)
        (let ((signaled
               (condition-case nil
                   (progn
                     (vtable-update-object table (list "new" 2)
                                          (list "missing" 9))
                     nil)
                 (error t))))
          (remacs-vtable3028-assert signaled
                                    "Missing object update did not signal")
          (remacs-vtable3028-assert
           (and (eq objects (vtable-objects table))
                (remacs-vtable3028-eq-list-p
                 (vtable-objects table) (list a b)))
           "Missing object update changed backing objects")
          (remacs-vtable3028-assert
           (and (eq cache (vtable--current-cache table))
                (eq lines (vtable--cache-lines cache))
                (eq line0 (car lines))
                (eq line1 (cadr lines))
                (remacs-vtable3028-eq-list-p (mapcar #'car lines) (list a b)))
           "Missing object update changed cache data")
          (let ((rows-after (remacs-vtable3028-display-rows table)))
            (remacs-vtable3028-assert
             (and (equal rows-before rows-after)
                  (remacs-vtable3028-eq-list-p
                   (mapcar #'car rows-after) (list a b)))
             "Missing object update changed displayed rows"))
          (remacs-vtable3028-assert
           (and (eq columns (vtable-columns table))
                (equal metadata
                       (mapcar (lambda (column)
                                 (list (vtable-column--numerical column)
                                       (vtable-column-align column)))
                               columns))
                (equal '((0 . ascend)) (vtable-sort-by table)))
           "Missing object update changed metadata or sort settings")
          (remacs-vtable3028-assert
           (and (eq (gethash key cache-table) cache)
                (eq (gethash other-key cache-table) other-cache))
           "Missing object update changed cached layouts")
          (remacs-vtable3028-assert
           (and (= getter-before remacs-vtable3028-getter-calls)
                (= formatter-before remacs-vtable3028-formatter-calls)
                (= displayer-before remacs-vtable3028-displayer-calls))
           "Missing object update called user functions")
          (remacs-vtable3028-assert
           (eq buffer-read-only t) "Missing update changed read-only state")
          ;; A cached model row without its displayed object property is also
          ;; unresolved: fail before changing model/cache or calling getters.
          (with-current-buffer (vtable-buffer table)
            (save-excursion
              (vtable-goto-table table)
              (when (vtable-goto-object b)
                (let ((inhibit-read-only t)
                      (inhibit-modification-hooks t))
                  (remove-text-properties
                   (line-beginning-position)
                   (min (point-max) (1+ (line-end-position)))
                   '(vtable-object nil))))))
          (let* ((lines-before (vtable--cache-lines cache))
                 (line0-before (car lines-before))
                 (line1-before (cadr lines-before))
                 (objects-before (vtable-objects table))
                 (cache-objects-before (mapcar #'car lines-before))
                 (text-before
                  (with-current-buffer (vtable-buffer table)
                    (buffer-substring-no-properties (point-min) (point-max))))
                 (rows-before (remacs-vtable3028-display-rows table))
                 (getter-count remacs-vtable3028-getter-calls)
                 (format-count remacs-vtable3028-formatter-calls)
                 (display-count remacs-vtable3028-displayer-calls)
                 (signaled
                  (condition-case nil
                      (progn
                        (vtable-update-object table (list "b" 2) b)
                        nil)
                    (error t))))
            (remacs-vtable3028-assert signaled
                                      "Missing display row did not signal")
            (remacs-vtable3028-assert
             (and (eq objects-before (vtable-objects table))
                  (remacs-vtable3028-eq-list-p
                   (vtable-objects table) (list a b)))
             "Missing display row changed backing objects")
            (remacs-vtable3028-assert
             (and (eq cache (vtable--current-cache table))
                  (eq lines-before (vtable--cache-lines cache))
                  (eq line0-before (car lines-before))
                  (eq line1-before (cadr lines-before))
                  (remacs-vtable3028-eq-list-p
                   (mapcar #'car lines-before) cache-objects-before))
             "Missing display row changed cache data")
            (remacs-vtable3028-assert
             (and (equal text-before
                         (with-current-buffer (vtable-buffer table)
                           (buffer-substring-no-properties
                            (point-min) (point-max))))
                  (equal rows-before
                         (remacs-vtable3028-display-rows table))
                  (null (with-current-buffer (vtable-buffer table)
                          (save-excursion
                            (vtable-goto-table table)
                            (vtable-goto-object b)))))
             "Missing display row was modified during failure")
            (remacs-vtable3028-assert
             (and (= getter-count remacs-vtable3028-getter-calls)
                  (= format-count remacs-vtable3028-formatter-calls)
                  (= display-count remacs-vtable3028-displayer-calls)
                  (eq (gethash key cache-table) cache)
                  (eq (gethash other-key cache-table) other-cache))
             "Missing display row failure called callbacks or invalidated cache")))
        t)))
  (unless (remacs-vtable3028-test-equal-copy-nonhead-update)
    (error "Equal-copy non-head update scenario failed"))
  (unless (remacs-vtable3028-test-eq-precedes-earlier-equal-object)
    (error "EQ precedence scenario failed"))
  (unless (remacs-vtable3028-test-equal-fallback-first-match)
    (error "Equal fallback duplicate scenario failed"))
  (unless (remacs-vtable3028-test-consecutive-eq-rows)
    (error "Consecutive EQ-row scenario failed"))
  (unless (remacs-vtable3028-test-adjacent-table-is-not-target)
    (error "Adjacent-table isolation scenario failed"))
  (unless (remacs-vtable3028-test-notfound-is-atomic)
    (error "Not-found atomicity scenario failed"))
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
fn vtable_update_object_matches_equal_old_object_3028() {
    assert_eq!(run_with_library("lisp/vtable.el"), "t");
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), "t");
}
