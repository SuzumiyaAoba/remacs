use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'vtable)
  (defvar remacs-vtable3033-getter-calls 0)
  (defvar remacs-vtable3033-object-queries 0)
  (defun remacs-vtable3033-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-vtable3033-getter (object index table)
    (setq remacs-vtable3033-getter-calls
          (1+ remacs-vtable3033-getter-calls))
    (aref object index))
  (defun remacs-vtable3033-row-objects ()
    (let (objects)
      (save-excursion
        (goto-char (point-min))
        (while (< (point) (point-max))
          (let ((object (get-text-property (line-end-position)
                                           'vtable-object)))
            (when object (push object objects)))
          (forward-line 1)))
      (nreverse objects)))
  (defun remacs-vtable3033-cache-values (table)
    (mapcar (lambda (line) (car (cadr line)))
            (vtable--cache-lines (vtable--current-cache table))))
  (defun remacs-vtable3033-check-order (text pieces)
    (let ((position 0)
          (matched t))
      (dolist (piece pieces)
        (let ((next (string-match (regexp-quote piece) text position)))
          (if next
              (setq position (+ next (length piece)))
            (setq matched nil))))
      matched))
  (defun remacs-vtable3033-default-header-case ()
    (with-temp-buffer
      (setq remacs-vtable3033-getter-calls 0)
      (insert "prefix\nsuffix\n")
      (goto-char (point-min))
      (forward-line 1)
      (let* ((insertion-point (point))
             (table (make-vtable :columns ["Name"] :objects nil
                                 :sort-by '((0 . ascend))
                                 :getter #'remacs-vtable3033-getter))
             (columns (vtable-columns table))
             (column (car columns))
             (contents-before (buffer-string))
             (calls-before remacs-vtable3033-getter-calls)
             (object (vector "first")))
        (remacs-vtable3033-assert
         (equal contents-before (buffer-string))
         "empty default-header table inserted buffer text")
        (remacs-vtable3033-assert
         (and (vtable-use-header-line table) header-line-format)
         "default header-line was not configured")
        (remacs-vtable3033-assert
         (= (vtable-goto-table table) insertion-point)
         "empty table goto did not use its saved insertion position")
        (remacs-vtable3033-assert
         (= (marker-position (slot-value table '-start-marker))
            insertion-point)
         "empty table marker moved before its first row")
        (setq buffer-read-only t)
        (vtable-insert-object table object)
        (let ((contents (buffer-string)))
          (remacs-vtable3033-assert
           (equal (vtable-objects table) (list object))
           "first insertion did not update object storage")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-cache-values table) '("first"))
           "first insertion did not fill the current cache")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-row-objects) (list object))
           "first insertion did not render at the saved boundary")
          (remacs-vtable3033-assert
           (remacs-vtable3033-check-order
            contents '("prefix" "first" "suffix"))
           "first row moved outside surrounding buffer text")
          (remacs-vtable3033-assert
           (= (marker-position (slot-value table '-start-marker))
              insertion-point)
           "table start marker moved to the end of its rendered row")
          (vtable-goto-table table)
          (remacs-vtable3033-assert
           (eq (vtable-current-table) table)
           "rendered row lost its table text property")
          (remacs-vtable3033-assert
           (and (not (vtable-column--numerical column))
                (eq (vtable-column-align column) 'left)
                (eq columns (vtable-columns table)))
           "first insertion did not preserve/update column metadata")
          (remacs-vtable3033-assert
           (equal (vtable-sort-by table) '((0 . ascend)))
           "first insertion changed sort settings")
          (remacs-vtable3033-assert
           (eq buffer-read-only t)
           "first insertion changed buffer read-only state")
          (remacs-vtable3033-assert
           (= remacs-vtable3033-getter-calls (+ calls-before 2))
           "first insertion getter count changed from preview plus render")
          t))))
  (defun remacs-vtable3033-inline-header-case ()
    (with-temp-buffer
      (setq remacs-vtable3033-getter-calls 0)
      (insert "prefix\nsuffix\n")
      (goto-char (point-min))
      (forward-line 1)
      (let* ((insertion-point (point))
             (table (make-vtable :columns ["Name"] :objects nil
                                 :use-header-line nil))
             (object (vector "inline")))
        (remacs-vtable3033-assert
         (= (vtable-goto-table table) insertion-point)
         "inline empty header lost its buffer boundary")
        (setq buffer-read-only t)
        (vtable-insert-object table object)
        (let ((contents (buffer-string)))
          (remacs-vtable3033-assert
           (remacs-vtable3033-check-order
            contents '("prefix" "Name" "inline" "suffix"))
           "inline-header first row was inserted at the wrong position")
          (remacs-vtable3033-assert
           (equal (vtable-objects table) (list object))
           "inline-header insertion did not update object storage")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-row-objects) (list object))
           "inline-header insertion did not render the row")
          (remacs-vtable3033-assert
           (eq buffer-read-only t)
           "inline-header insertion changed read-only state")
          t))))
  (defun remacs-vtable3033-insert-nil-case ()
    (with-temp-buffer
      (setq remacs-vtable3033-getter-calls 0)
      (insert "prefix\nsuffix\n")
      (goto-char (point-min))
      (forward-line 1)
      (let* ((insertion-point (point))
             (table (make-vtable :columns ["Name"] :objects nil
                                 :sort-by '((0 . ascend))
                                 :getter #'remacs-vtable3033-getter
                                 :insert nil))
             (columns (vtable-columns table))
             (column (car columns))
             (first (vector "first"))
             (second (vector "second"))
             (contents-before (buffer-string)))
        (vtable-insert-object table first)
        (vtable-insert-object table second)
        (remacs-vtable3033-assert
         (null (vtable-buffer table))
         "INSERT=nil data-only insertion unexpectedly attached a buffer")
        (remacs-vtable3033-assert
         (equal (vtable-objects table) (list first second))
         "INSERT=nil multiple insertions lost object order")
        (remacs-vtable3033-assert
         (equal contents-before (buffer-string))
         "INSERT=nil insertion changed the buffer before rendering")
        (remacs-vtable3033-assert
         (= (point) insertion-point)
         "INSERT=nil insertion moved the caller's insertion point")
        (remacs-vtable3033-assert
         (and (not (vtable-column--numerical column))
              (eq (vtable-column-align column) 'left))
         "INSERT=nil did not keep column metadata current")
        (setq buffer-read-only t)
        (vtable-insert table)
        (let ((contents (buffer-string)))
          (remacs-vtable3033-assert
           (equal (vtable-objects table) (list first second))
           "later vtable-insert changed the deferred object list")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-cache-values table)
                  '("first" "second"))
           "later vtable-insert did not build the deferred cache")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-row-objects) (list first second))
           "later vtable-insert did not render deferred rows")
          (remacs-vtable3033-assert
           (remacs-vtable3033-check-order
            contents '("prefix" "first" "second" "suffix"))
           "deferred table moved outside surrounding buffer text")
          (remacs-vtable3033-assert
           (eq buffer-read-only t)
           "deferred insertion changed buffer read-only state")
          (remacs-vtable3033-assert
           (equal (vtable-sort-by table) '((0 . ascend)))
           "deferred insertion changed sort settings")
          (remacs-vtable3033-assert
           (eq columns (vtable-columns table))
           "deferred insertion changed column identity")
          (remacs-vtable3033-assert
           (= remacs-vtable3033-getter-calls 4)
           "INSERT=nil two-row prepopulate/render getter count changed")
          t))))
  (remacs-vtable3033-assert
   (remacs-vtable3033-default-header-case)
   "default-header empty-table first insert failed")
  (remacs-vtable3033-assert
   (remacs-vtable3033-inline-header-case)
   "inline-header empty-table first insert failed")
  (remacs-vtable3033-assert
   (remacs-vtable3033-insert-nil-case)
   "INSERT=nil prepopulate then render failed")
  (defun remacs-vtable3033-crossreview-integer-before-case ()
    (with-temp-buffer
      (setq remacs-vtable3033-getter-calls 0)
      (let* ((first (vector "first"))
             (second (vector "second"))
             (third (vector "third"))
             (table (make-vtable :columns ["Name"]
                                 :objects (list first second)
                                 :getter #'remacs-vtable3033-getter))
             (cache (vtable--current-cache table))
             (columns (vtable-columns table))
             (sort-before (vtable-sort-by table))
             (calls-before remacs-vtable3033-getter-calls))
        (setq buffer-read-only t)
        (let ((result (vtable-insert-object table third 99 t)))
          (remacs-vtable3033-assert
           (equal (vtable-objects table) (list first second third))
           "out-of-range integer LOCATION with BEFORE did not append to objects")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-cache-values table)
                  '("first" "second" "third"))
           "out-of-range integer LOCATION with BEFORE corrupted cache order")
          (remacs-vtable3033-assert
           (equal (remacs-vtable3033-row-objects)
                  (list first second third))
           "out-of-range integer LOCATION with BEFORE corrupted display order")
          (remacs-vtable3033-assert
           (eq cache (vtable--current-cache table))
           "integer insertion replaced the active cache")
          (remacs-vtable3033-assert
           (eq columns (vtable-columns table))
           "integer insertion changed column identity")
          (remacs-vtable3033-assert
           (equal sort-before (vtable-sort-by table))
           "integer insertion changed sorting")
          (remacs-vtable3033-assert
           (= remacs-vtable3033-getter-calls (1+ calls-before))
           "integer insertion reran the getter")
          (remacs-vtable3033-assert
           (eq buffer-read-only t)
           "integer insertion changed the read-only state")
          (remacs-vtable3033-assert
           (null result)
           "integer insertion changed the existing numerical-recompute return")
          t))))
  (defun remacs-vtable3033-crossreview-shared-marker-case ()
    (with-temp-buffer
      (insert "prefix\nsuffix\n")
      (goto-char (point-min))
      (forward-line 1)
      (let* ((start (point))
             (first (make-vtable :columns ["First"] :objects nil))
             (second (make-vtable :columns ["Second"] :objects nil))
             (first-marker (slot-value first '-start-marker))
             (second-marker (slot-value second '-start-marker)))
        (remacs-vtable3033-assert
         (and (= (marker-position first-marker) start)
              (= (marker-position second-marker) start)
              (marker-insertion-type first-marker)
              (marker-insertion-type second-marker))
         "empty tables did not retain distinct insertion-type markers")
        (goto-char first-marker)
        (insert "intervening\n")
        (remacs-vtable3033-assert
         (and (= (marker-position first-marker) (point))
              (= (marker-position second-marker) (point)))
         "markers did not move after text inserted at their shared boundary")
        (vtable-insert-object first (vector "first"))
        (remacs-vtable3033-assert
         (eq first-marker (slot-value first '-start-marker))
         "first table replaced its marker object during render")
        (let ((second-start (marker-position second-marker)))
          (remacs-vtable3033-assert
           (> second-start (marker-position first-marker))
           "rendering the first table did not advance the second marker")
          (vtable-insert-object second (vector "second"))
          (remacs-vtable3033-assert
           (eq second-marker (slot-value second '-start-marker))
           "second table replaced its marker object during render")
          (remacs-vtable3033-assert
           (remacs-vtable3033-check-order
            (buffer-string)
            '("prefix" "intervening" "first" "second" "suffix"))
           "two empty tables at one boundary rendered in the wrong order")
          (remacs-vtable3033-assert
           (= (marker-position second-marker) second-start)
           "rendering the second table moved its start marker to its end")
          t))))
  (defun remacs-vtable3033-crossreview-marker-outside-narrowing-case ()
    (with-temp-buffer
      (insert "prefix\nsuffix\n")
      (goto-char (point-min))
      (forward-line 1)
      (let* ((table (make-vtable :columns ["Name"] :objects nil))
             (cache (vtable--current-cache table))
             (marker-position-before
              (marker-position (slot-value table '-start-marker)))
             (contents-before (buffer-substring-no-properties
                               (point-min) (point-max))))
        (save-restriction
          (narrow-to-region (1+ marker-position-before) (point-max))
          (let ((point-before (point))
                (accessible-before (buffer-substring-no-properties
                                   (point-min) (point-max))))
            (remacs-vtable3033-assert
             (null (vtable-goto-table table))
             "goto-table treated a marker outside narrowing as accessible")
            (remacs-vtable3033-assert
             (= (point) point-before)
             "failed goto-table moved point under narrowing")
            (remacs-vtable3033-assert
             (condition-case nil
                 (progn (vtable-insert-object table (vector "inaccessible")) nil)
               (user-error t))
             "insert-object modified a table outside narrowing")
            (remacs-vtable3033-assert
             (null (vtable-objects table))
             "inaccessible insert-object changed backing objects")
            (remacs-vtable3033-assert
             (condition-case nil
                 (progn (vtable-revert table) nil)
               (user-error t))
             "revert did not reject an inaccessible insertion marker")
            (remacs-vtable3033-assert
             (equal accessible-before
                    (buffer-substring-no-properties (point-min) (point-max)))
             "failed narrowed revert changed accessible text")
            (remacs-vtable3033-assert
             (eq cache (vtable--current-cache table))
             "failed narrowed revert changed the active cache")
            (remacs-vtable3033-assert
             (condition-case nil
                 (progn (vtable-revert-command table) nil)
               (user-error t))
             "revert-command did not reject an inaccessible marker")
            (remacs-vtable3033-assert
             (eq cache (vtable--current-cache table))
             "failed revert-command evicted the active cache")))
        (remacs-vtable3033-assert
         (equal contents-before
                (buffer-substring-no-properties (point-min) (point-max)))
         "failed narrowed revert changed surrounding buffer text")
        t)))
  (defun remacs-vtable3033-crossreview-partial-narrowing-case ()
    (with-temp-buffer
      (insert "prefix\n")
      (let* ((first (vector "first"))
             (second (vector "second"))
             (objects (list first second))
             (table (make-vtable :columns ["Name"] :objects objects
                                 :use-header-line nil))
             (cache (vtable--current-cache table))
             (cache-key (slot-value table '-current-cache-key)))
        (goto-char (point-max))
        (insert "suffix\n")
        (let* ((start (save-excursion
                        (vtable-goto-table table)
                        (point)))
               (partial-end (save-excursion
                              (goto-char start)
                              (forward-line 1)
                              (point)))
               (contents-before
                (buffer-substring-no-properties (point-min) (point-max)))
               (objects-before (copy-sequence (vtable-objects table))))
          (save-restriction
            (narrow-to-region start partial-end)
            (let ((accessible-before
                   (buffer-substring-no-properties (point-min) (point-max))))
              (remacs-vtable3033-assert
               (condition-case nil
                   (progn (vtable-revert-command table) nil)
                 (user-error t))
               "revert-command accepted a restriction covering only part of a table")
              (remacs-vtable3033-assert
               (equal accessible-before
                      (buffer-substring-no-properties (point-min) (point-max)))
               "partial-table revert changed accessible text")
              (remacs-vtable3033-assert
               (condition-case nil
                   (progn (vtable-insert-object table (vector "third")) nil)
                 (user-error t))
               "insert-object accepted a restriction covering only part of a table")
              (remacs-vtable3033-assert
               (eq cache (vtable--current-cache table))
               "partial-table insert-object changed the current cache")
              (remacs-vtable3033-assert
               (equal objects-before (vtable-objects table))
               "partial-table insert-object changed backing objects")
              (remacs-vtable3033-assert
               (eq cache (gethash cache-key (slot-value table '-cache)))
               "partial-table revert changed the keyed cache")
              (remacs-vtable3033-assert
               (equal objects-before (vtable-objects table))
               "partial-table revert changed backing objects")))
          (remacs-vtable3033-assert
           (equal contents-before
                  (buffer-substring-no-properties (point-min) (point-max)))
           "partial-table revert changed text outside the narrowed region")
          t))))
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-partial-narrowing-case)
   "partial-table narrowing regression")
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-integer-before-case)
   "integer LOCATION must override BEFORE")
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-shared-marker-case)
   "shared empty-table markers regression")
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-marker-outside-narrowing-case)
   "inaccessible marker narrowing regression")
(defun remacs-vtable3033-crossreview-revert-narrowing-case ()
    (with-temp-buffer
      (insert "prefix\n")
      (let* ((object (vector "row"))
             (table (make-vtable :columns ["N"] :objects (list object)
                                 :use-header-line nil)))
        (goto-char (point-max))
        (insert "suffix\n")
        (vtable-goto-table table)
        (vtable-goto-object object)
        (vtable-goto-column 0)
        (let* ((start (save-excursion
                        (vtable-goto-table table)
                        (point)))
               (end (save-excursion
                      (goto-char start)
                      (vtable--end-of-table table))))
          (save-restriction
            (narrow-to-region start end)
            (vtable-revert table)
            (remacs-vtable3033-assert
             (buffer-narrowed-p)
             "revert widened a buffer narrowed to the complete table")
            (remacs-vtable3033-assert
             (eq (vtable-current-object) object)
             "revert did not restore the current row")
            (remacs-vtable3033-assert
             (= (vtable-current-column) 0)
             "revert did not restore the current column"))
          (vtable-revert table)
          (remacs-vtable3033-assert
           (remacs-vtable3033-check-order
            (buffer-string) '("prefix" "N" "row" "suffix"))
           "repeated revert changed text surrounding the table")
          (remacs-vtable3033-assert
           (= (how-many "row" (point-min) (point-max)) 1)
           "repeated revert duplicated the displayed row")
          t))))
(defun remacs-vtable3033-crossreview-deleted-render-case ()
    (with-temp-buffer
      (insert "prefix\n")
      (let* ((object (vector "row"))
             (table (make-vtable :columns ["N"] :objects (list object)
                                 :use-header-line nil)))
        (goto-char (point-max))
        (insert "suffix\n")
        (let* ((start (save-excursion
                        (vtable-goto-table table)
                        (point)))
               (end (save-excursion
                      (goto-char start)
                      (vtable--end-of-table table)))
               (marker (slot-value table '-start-marker)))
          (delete-region start end)
          (remacs-vtable3033-assert
           (= (marker-position marker) start)
           "deleting rendered text did not leave its insertion boundary")
          (remacs-vtable3033-assert
           (= (vtable-goto-table table) start)
           "deleted live table could not be found at its marker")
          (vtable-revert table)
          (remacs-vtable3033-assert
           (remacs-vtable3033-check-order
            (buffer-string) '("prefix" "N" "row" "suffix"))
           "revert after live text deletion duplicated or misplaced the table")
          t))))
(defun remacs-vtable3033-crossreview-insert-nil-cross-buffer-case ()
    (setq remacs-vtable3033-getter-calls 0)
    (let* ((table
            (with-temp-buffer
              (make-vtable :columns ["Name"] :objects nil
                           :getter #'remacs-vtable3033-getter
                           :use-header-line nil :insert nil)))
           (target (generate-new-buffer " *vtable-3033-cross-buffer*")))
      (unwind-protect
          (with-current-buffer target
            (insert "prefix\nsuffix\n")
            (goto-char (point-min))
            (forward-line 1)
            (let* ((object (vector "deferred"))
                   (contents-before (buffer-string))
                   (point-before (point)))
              (vtable-insert-object table object)
              (remacs-vtable3033-assert
               (null (vtable-buffer table))
               "INSERT=nil prepopulation attached the source buffer")
              (remacs-vtable3033-assert
               (and (equal contents-before (buffer-string))
                    (= point-before (point)))
               "INSERT=nil prepopulation changed the destination buffer")
              (vtable-insert table)
              (remacs-vtable3033-assert
               (eq (vtable-buffer table) (current-buffer))
               "deferred insert did not attach to the chosen destination buffer")
              (remacs-vtable3033-assert
               (and (equal (vtable-objects table) (list object))
                    (equal (remacs-vtable3033-cache-values table) '("deferred"))
                    (equal (remacs-vtable3033-row-objects) (list object)))
               "cross-buffer deferred render lost model/cache/display data")
              (remacs-vtable3033-assert
               (remacs-vtable3033-check-order
                (buffer-string) '("prefix" "Name" "deferred" "suffix"))
               "cross-buffer deferred render moved surrounding text")
              (remacs-vtable3033-assert
               (= remacs-vtable3033-getter-calls 2)
               "cross-buffer deferred render getter count changed")
              t))
        (kill-buffer target))))
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-revert-narrowing-case)
   "revert/narrowing regression")
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-deleted-render-case)
   "deleted table recovery regression")
  (remacs-vtable3033-assert
   (remacs-vtable3033-crossreview-insert-nil-cross-buffer-case)
   "cross-buffer insert-nil regression")
  (with-temp-buffer
    (let* ((remacs-vtable3033-getter-calls 0)
           (remacs-vtable3033-object-queries 0)
           (object (vector "factory-first"))
           (table (make-vtable
                   :columns ["Name"]
                   :objects-function
                   (lambda ()
                     (setq remacs-vtable3033-object-queries
                           (1+ remacs-vtable3033-object-queries))
                     nil)
                   :getter #'remacs-vtable3033-getter)))
      (setq buffer-read-only t)
      (vtable-insert-object table object)
      (remacs-vtable3033-assert
       (and (equal (vtable-objects table) (list object))
            (equal (remacs-vtable3033-cache-values table) '("factory-first"))
            (equal (remacs-vtable3033-row-objects) (list object))
            (= remacs-vtable3033-object-queries 1)
            (= remacs-vtable3033-getter-calls 2)
            buffer-read-only)
       "first insertion re-queried and discarded its supplied object")
      (vtable-revert-command table)
      (remacs-vtable3033-assert
       (and (= remacs-vtable3033-object-queries 2)
            (null (vtable-objects table))
            (null (remacs-vtable3033-row-objects))
            buffer-read-only)
       "explicit revert no longer re-queries the object supplier")))
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
fn vtable_insert_object_handles_zero_width_and_unrendered_tables() {
    assert_eq!(run_with_library("lisp/vtable.el"), "t");
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), "t");
}
