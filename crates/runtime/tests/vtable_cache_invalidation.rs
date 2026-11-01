use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'vtable)
  (defvar remacs-vtable-crossreview-getter-calls 0)
  (defun remacs-vtable-crossreview-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-vtable-crossreview-cache-values (cache)
    (mapcar (lambda (line) (car (cadr line)))
            (vtable--cache-lines cache)))
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
  (defun remacs-vtable-crossreview-getter (object index table)
    (setq remacs-vtable-crossreview-getter-calls
          (1+ remacs-vtable-crossreview-getter-calls))
    (aref object index))
  (defun remacs-vtable-crossreview-render-other-width (table key)
    (with-current-buffer (vtable-buffer table)
      (let ((inhibit-read-only t))
        (erase-buffer))
      (cl-letf (((symbol-function 'vtable--cache-key)
                 (lambda (&optional _table) key)))
        (vtable-insert table))))
  (defun remacs-vtable-crossreview-update-shared-cache ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((old (vector 1))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list old)
                                 :sort-by '((0 . ascend))
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (current-cache (gethash key cache-table))
             (columns (vtable-columns table))
             (column (car columns))
             (sort-before (vtable-sort-by table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        ;; Deliberately share one cache object under two distinct width keys.
        (puthash other-key current-cache cache-table)
        (setq buffer-read-only t)
        (let ((update-result (vtable-update-object table new old)))
          (remacs-vtable-crossreview-assert
           (null (gethash other-key cache-table))
           "update did not evict another key that aliased the current cache")
          (remacs-vtable-crossreview-assert
           (eq (gethash key cache-table) current-cache)
           "update replaced the current width cache")
          (remacs-vtable-crossreview-assert
           (eq (vtable--current-cache table) current-cache)
           "update changed the active cache object")
          (remacs-vtable-crossreview-assert
           (equal (remacs-vtable-crossreview-cache-values current-cache)
                  '("new"))
           "current cache did not receive the updated row")
          (remacs-vtable-crossreview-assert
           (eq update-result columns)
           "update return value changed when numerical metadata was recomputed")
          (remacs-vtable-crossreview-assert
           (and (eq (car (vtable-objects table)) new)
                (not (memq old (vtable-objects table))))
           "object storage was not updated")
          (remacs-vtable-crossreview-assert
           (and (not (vtable-column--numerical column))
                (eq (vtable-column-align column) 'left)
                (not (vtable-column--aligned column)))
           "numerical/alignment metadata was not recomputed")
          (remacs-vtable-crossreview-assert
           (eq columns (vtable-columns table))
           "column collection identity changed")
          (remacs-vtable-crossreview-assert
           (equal sort-before (vtable-sort-by table))
           "sort settings changed")
          (remacs-vtable-crossreview-assert
           (eq buffer-read-only t) "buffer read-only state changed")
          (remacs-vtable-crossreview-assert
           (equal (remacs-vtable-crossreview-row-objects) (list new))
           "updated display row is missing")
          (remacs-vtable-crossreview-assert
           (= remacs-vtable-crossreview-getter-calls
              (1+ calls-before))
           "update ran the getter more than once for the new row")
          (remacs-vtable-crossreview-render-other-width table other-key)
          (remacs-vtable-crossreview-assert
           (equal (remacs-vtable-crossreview-cache-values
                   (gethash other-key cache-table))
                  '("new"))
           "other-width update render used stale shared-cache data")
          (remacs-vtable-crossreview-assert
           (= remacs-vtable-crossreview-getter-calls
              (+ calls-before 2))
           "other-width rebuild ran its getter more than once")
          t))))
  (defun remacs-vtable-crossreview-revert-shared-cache ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((object (vector "same"))
             (table (make-vtable :columns ["Name"] :objects (list object)
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (current-cache (gethash key cache-table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        (puthash other-key current-cache cache-table)
        (setq buffer-read-only t)
        (vtable-revert table)
        (remacs-vtable-crossreview-assert
         (null (gethash other-key cache-table))
         "direct revert retained an alias under another width key")
        (remacs-vtable-crossreview-assert
         (eq (gethash key cache-table) current-cache)
         "direct revert replaced its current-width cache")
        (remacs-vtable-crossreview-assert
         (eq (vtable--current-cache table) current-cache)
         "direct revert changed the current cache pointer")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values current-cache)
                '("same"))
         "direct revert changed current cache data")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-row-objects) (list object))
         "direct revert changed the rendered row")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls calls-before)
         "direct revert re-ran cached getters")
        (remacs-vtable-crossreview-assert
         (eq buffer-read-only t) "direct revert changed read-only state")
        t)))
  (defun remacs-vtable-crossreview-revert-with-nil-current-cache ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((old (vector "old"))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list old)
                                 :sort-by '((0 . ascend))
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (column (car (vtable-columns table)))
             (sort-before (vtable-sort-by table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        (puthash other-key (copy-tree (gethash key cache-table)) cache-table)
        (puthash nil (copy-tree (gethash key cache-table)) cache-table)
        ;; This width has no cached data, while another width is stale.
        (puthash key nil cache-table)
        (setf (vtable-objects table) (list new))
        (setq buffer-read-only t)
        (vtable-revert-command table)
        (remacs-vtable-crossreview-assert
         (null (gethash other-key cache-table))
         "revert left a stale cache when the current key held nil")
        (remacs-vtable-crossreview-assert
         (null (gethash nil cache-table))
         "revert with no cache to preserve did not discard every key")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values
                 (gethash key cache-table))
                '("new"))
         "revert did not rebuild the current cache")
        (remacs-vtable-crossreview-assert
         (eq (vtable--current-cache table) (gethash key cache-table))
         "revert's current cache pointer differs from its keyed cache")
        (remacs-vtable-crossreview-assert
         (and (eq (car (vtable-objects table)) new)
              (not (memq old (vtable-objects table))))
         "revert left stale object storage")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-row-objects) (list new))
         "revert left stale display rows")
        (remacs-vtable-crossreview-assert
         (eq (vtable-column-align column) 'left)
         "revert changed string-column alignment")
        (remacs-vtable-crossreview-assert
         (equal sort-before (vtable-sort-by table))
         "revert changed sort settings")
        (remacs-vtable-crossreview-assert
         (eq buffer-read-only t) "revert changed buffer read-only state")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls
            (1+ calls-before))
         "revert re-ran the getter beyond the single refreshed row")
        (remacs-vtable-crossreview-render-other-width table other-key)
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values
                 (gethash other-key cache-table))
                '("new"))
         "nil-current-cache revert rebuilt a stale width")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls
            (+ calls-before 2))
         "other-width rebuild ran its getter more than once")
        t)))
  (defun remacs-vtable-crossreview-revert-preserves-sort-order ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((beta (vector "beta"))
             (alpha (vector "alpha"))
             (gamma (vector "gamma"))
             (table (make-vtable :columns ["Name"] :objects (list beta alpha)
                                 :sort-by '((0 . ascend))
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (sort-before (vtable-sort-by table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        (puthash other-key
                 (copy-tree (gethash key cache-table))
                 cache-table)
        (setf (vtable-objects table) (list gamma beta))
        (setq buffer-read-only t)
        (vtable-revert-command table)
        (remacs-vtable-crossreview-assert
         (null (gethash other-key cache-table))
         "sorted revert retained the old other-width cache")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values
                 (gethash key cache-table))
                '("beta" "gamma"))
         "current-width cache was not sorted after revert")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-row-objects) (list beta gamma))
         "current-width display did not retain sort order")
        (remacs-vtable-crossreview-assert
         (equal (vtable-objects table) (list gamma beta))
         "revert reordered the source object list")
        (remacs-vtable-crossreview-assert
         (equal (vtable-sort-by table) sort-before)
         "revert changed sort settings")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls (+ calls-before 2))
         "current-width sorted rebuild ran getters unexpectedly")
        (remacs-vtable-crossreview-render-other-width table other-key)
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values
                 (gethash other-key cache-table))
                '("beta" "gamma"))
         "other-width cache was not rebuilt in sort order")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-row-objects) (list beta gamma))
         "other-width display did not retain sort order")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls (+ calls-before 4))
         "other-width sorted rebuild ran getters unexpectedly")
        (remacs-vtable-crossreview-assert
         (eq buffer-read-only t) "sorted revert changed read-only state")
        t)))
  (defun remacs-vtable-crossreview-insert-first-row ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects nil
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (column (car (vtable-columns table)))
             (empty-cache (gethash key cache-table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        ;; A second width has cached the empty table.
        (puthash other-key (copy-tree empty-cache) cache-table)
        (setq buffer-read-only t)
        (vtable-insert-object table new)
        (remacs-vtable-crossreview-assert
         (equal (vtable-objects table) (list new))
         "first-row insertion did not update object storage")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values
                 (gethash key cache-table))
                '("new"))
         "first-row insertion did not populate the current cache")
        (remacs-vtable-crossreview-assert
         (null (gethash other-key cache-table))
         "first-row insertion retained another width's empty cache")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-row-objects) (list new))
         "first-row insertion did not update the display")
        (remacs-vtable-crossreview-assert
         (and (not (vtable-column--numerical column))
              (eq (vtable-column-align column) 'left))
         "first-row insertion did not update column metadata")
        (remacs-vtable-crossreview-assert
         (eq buffer-read-only t) "first-row insertion changed read-only state")
        ;; The existing empty-table path computes a preview and then reverts.
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls (+ calls-before 2))
         "empty insertion getter-count changed from the existing preview/revert path")
        (remacs-vtable-crossreview-render-other-width table other-key)
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values
                 (gethash other-key cache-table))
                '("new"))
         "empty insertion's other-width cache was not rebuilt")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls (+ calls-before 3))
         "empty insertion other-width rebuild ran its getter more than once")
        (list (- remacs-vtable-crossreview-getter-calls calls-before)
              (slot-value table '-current-cache-key)
              other-key))))
  (defun remacs-vtable-crossreview-remove-invalidates-other-width ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((same (vector "same"))
             (equal-but-distinct (vector "same"))
             (table (make-vtable :columns ["Name"]
                                 :objects (list same same equal-but-distinct)
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (current-cache (gethash key cache-table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        (puthash other-key (copy-tree current-cache) cache-table)
        (setq buffer-read-only t)
        (let ((result (vtable-remove-object table same)))
          (remacs-vtable-crossreview-assert
           (null (gethash other-key cache-table))
           "remove retained another width's cache")
          (remacs-vtable-crossreview-assert
           (eq (vtable--current-cache table) current-cache)
           "remove replaced the active cache")
          (remacs-vtable-crossreview-assert
           (equal (remacs-vtable-crossreview-cache-values current-cache)
                  '("same"))
           "remove changed the surviving cache row")
          (remacs-vtable-crossreview-assert
           (equal (vtable-objects table) (list equal-but-distinct))
           "remove did not preserve the equal-but-distinct object")
          (remacs-vtable-crossreview-assert
           (equal (remacs-vtable-crossreview-row-objects)
                  (list equal-but-distinct))
           "remove left stale duplicate display rows")
          (remacs-vtable-crossreview-assert
           (null result) "remove return value changed")
          (remacs-vtable-crossreview-assert
           (= remacs-vtable-crossreview-getter-calls calls-before)
           "remove re-ran getters")
          (remacs-vtable-crossreview-assert
           (eq buffer-read-only t) "remove changed read-only state")
          t))))
  (defun remacs-vtable-crossreview-insert-nonempty-invalidates-other-width ()
    (with-temp-buffer
      (setq remacs-vtable-crossreview-getter-calls 0)
      (let* ((old (vector 2))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list old)
                                 :use-header-line nil
                                 :getter #'remacs-vtable-crossreview-getter))
             (cache-table (slot-value table '-cache))
             (key (vtable--cache-key table))
             (other-key (cons (car key) (1+ (cdr key))))
             (current-cache (gethash key cache-table))
             (columns (vtable-columns table))
             (column (car columns))
             (sort-before (vtable-sort-by table))
             (calls-before remacs-vtable-crossreview-getter-calls))
        (puthash other-key (copy-tree current-cache) cache-table)
        (setq buffer-read-only t)
        (let ((insert-result (vtable-insert-object table new old)))
        (remacs-vtable-crossreview-assert
         (null (gethash other-key cache-table))
         "nonempty insertion retained another width's cache")
        (remacs-vtable-crossreview-assert
         (eq (gethash key cache-table) current-cache)
         "nonempty insertion replaced the current cache")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-cache-values current-cache)
                '(2 "new"))
         "nonempty insertion did not update current cache rows")
        (remacs-vtable-crossreview-assert
         (equal (vtable-objects table) (list old new))
         "nonempty insertion changed object order")
        (remacs-vtable-crossreview-assert
         (equal (remacs-vtable-crossreview-row-objects) (list old new))
         "nonempty insertion did not update displayed rows")
        (remacs-vtable-crossreview-assert
         (eq columns insert-result)
         "nonempty insertion return value changed")
        (remacs-vtable-crossreview-assert
         (eq columns (vtable-columns table))
         "nonempty insertion changed column identity")
        (remacs-vtable-crossreview-assert
         (and (not (vtable-column--numerical column))
              (eq (vtable-column-align column) 'left))
         "nonempty insertion did not update column metadata")
        (remacs-vtable-crossreview-assert
         (equal sort-before (vtable-sort-by table))
         "nonempty insertion changed sort settings")
        (remacs-vtable-crossreview-assert
         (= remacs-vtable-crossreview-getter-calls (+ calls-before 1))
         "nonempty insertion ran the new row getter more than once")
        (remacs-vtable-crossreview-assert
         (eq buffer-read-only t) "nonempty insertion changed read-only state")
        t))))
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-remove-invalidates-other-width)
   "remove other-width invalidation regression failed")
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-insert-nonempty-invalidates-other-width)
   "nonempty insert other-width invalidation regression failed")
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-update-shared-cache)
   "shared-cache update regression failed")
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-revert-shared-cache)
   "direct revert shared-cache regression failed")
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-revert-preserves-sort-order)
   "sorted multi-width revert regression failed")
  (remacs-vtable-crossreview-assert
   (remacs-vtable-crossreview-revert-with-nil-current-cache)
   "nil-current-cache revert regression failed")
  (let ((empty-result (remacs-vtable-crossreview-insert-first-row)))
    (remacs-vtable-crossreview-assert
     (equal (car empty-result) 3)
     "empty insertion plus other-width rebuild getter count changed")
    (remacs-vtable-crossreview-assert
     (equal (cadr empty-result) (caddr empty-result))
     "rendered cache key does not match its current width key"))
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
fn vtable_cache_invalidation_handles_alias_nil_and_empty_caches() {
    assert_eq!(run_with_library("lisp/vtable.el"), "t");
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), "t");
}
