use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'vtable)
  (defvar remacs-vtable3030-getter-calls 0)
  (defvar remacs-vtable3030-formatter-calls 0)
  (defvar remacs-vtable3030-displayer-calls 0)
  (defun remacs-vtable3030-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-vtable3030-getter (object index _table)
    (setq remacs-vtable3030-getter-calls
          (1+ remacs-vtable3030-getter-calls))
    (aref object index))
  (defun remacs-vtable3030-formatter (value _index _table)
    (setq remacs-vtable3030-formatter-calls
          (1+ remacs-vtable3030-formatter-calls))
    value)
  (defun remacs-vtable3030-displayer (value _index _width _table)
    (setq remacs-vtable3030-displayer-calls
          (1+ remacs-vtable3030-displayer-calls))
    value)
  (defun remacs-vtable3030-face-has-p (face expected)
    (or (equal face expected)
        (cl-member expected (vtable--face-components face) :test #'equal)))
  (defun remacs-vtable3030-face-count (face expected)
    (cl-count expected (vtable--face-components face) :test #'equal))
  (defun remacs-vtable3030-row-snapshot (table &optional column)
    (with-current-buffer (vtable-buffer table)
      (save-excursion
        (unless (vtable-goto-table table)
          (error "Could not find vtable in its buffer"))
        (when (and (slot-value table '-has-column-spec)
                   (not (vtable-use-header-line table)))
          (forward-line 1))
        (let (rows)
          (while (< (point) (point-max))
            (let* ((row-start (line-beginning-position))
                   (row-end (line-end-position))
                   (object (get-text-property row-end 'vtable-object))
                   (face-position
                    (if (numberp column)
                        (save-excursion
                          (vtable-goto-column column)
                          (point))
                      row-end))
                   (face (get-text-property face-position 'face))
                   (next-face
                    (and (< (1+ face-position) row-end)
                         (get-text-property (1+ face-position) 'face)))
                   (text (buffer-substring-no-properties row-start row-end))
                   (base-face
                    (get-text-property face-position
                                       'vtable--row-color-face-record))
                   (display (get-text-property face-position 'display))
                   (next-display
                    (and (< (1+ face-position) row-end)
                         (get-text-property (1+ face-position) 'display)))
                   )
              (when object
                (push (list object face text base-face display
                            next-face next-display)
                      rows)))
            (forward-line 1))
          (nreverse rows)))))
  (defun remacs-vtable3030-check-row-color (row color)
    (remacs-vtable3030-assert
     (remacs-vtable3030-face-has-p (cadr row) color)
     (format "Row %S lacks color face %S: %S" (car row) color (cadr row))))
  (defun remacs-vtable3030-check-blend-colors (table rows column)
    (let ((colors (slot-value table '-cached-colors))
          (row-count (length (vtable-row-colors table)))
          (index 0))
      (dolist (row rows)
        (let ((expected (elt (elt colors (mod index row-count)) column)))
          (remacs-vtable3030-assert
           (remacs-vtable3030-face-has-p (cadr row) expected)
           (format "Column %d row %d lost blended color: %S != %S"
                   column index (cadr row) expected)))
        (setq index (1+ index)))))
  (defun remacs-vtable3030-logical-row-snapshot (table)
    (with-current-buffer (vtable-buffer table)
      (save-excursion
        (unless (vtable-goto-table table)
          (error "Could not find vtable in its buffer"))
        (when (and (slot-value table '-has-column-spec)
                   (not (vtable-use-header-line table)))
          (forward-line 1))
        (let ((table-end (save-excursion (vtable-end-of-table))) rows)
          (while (< (point) table-end)
            (let* ((token
                    (get-text-property (point)
                                       'vtable--row-color-row-token))
                   (next
                    (or (next-single-property-change
                         (point) 'vtable--row-color-row-token nil table-end)
                        table-end)))
              (if token
                  (let* ((start (point))
                         (object (get-text-property (1- next) 'vtable-object))
                         (face (get-text-property start 'face))
                         (base-face
                          (get-text-property
                           start 'vtable--row-color-face-record))
                         (text (buffer-substring-no-properties start next)))
                    (when object
                      (push (list object face text base-face nil nil nil token) rows))
                    (goto-char next))
                (goto-char next))))
          (nreverse rows)))))
  (defun remacs-vtable3030-check-logical-row-span-face
      (table object expected)
    (with-current-buffer (vtable-buffer table)
      (save-excursion
        (unless (vtable-goto-table table)
          (error "Could not find vtable in its buffer"))
        (when (and (slot-value table '-has-column-spec)
                   (not (vtable-use-header-line table)))
          (forward-line 1))
        (let ((table-end (save-excursion (vtable-end-of-table)))
              (found nil))
          (while (< (point) table-end)
            (let* ((token
                    (get-text-property (point)
                                       'vtable--row-color-row-token))
                   (next
                    (or (next-single-property-change
                         (point) 'vtable--row-color-row-token nil table-end)
                        table-end)))
              (when (and token
                         (eq (get-text-property (1- next) 'vtable-object)
                             object))
                (setq found t)
                (let ((position (point)))
                  (while (< position next)
                    (unless (= (char-after position) 10)
                      (remacs-vtable3030-assert
                       (remacs-vtable3030-face-has-p
                        (get-text-property position 'face) expected)
                       (format "Logical row %S lost face %S at %d"
                               object expected position)))
                    (setq position (1+ position)))))
              (goto-char next)))
          (remacs-vtable3030-assert found
                                    (format "Logical row %S was not found"
                                            object))
          t))))
  (defun remacs-vtable3030-cell-face-state (table object offset)
    (with-current-buffer (vtable-buffer table)
      (save-excursion
        (unless (vtable-goto-object object)
          (error "Could not find object %S" object))
        (vtable-goto-column 0)
        (forward-char offset)
        (list (get-text-property (point) 'face)
              (get-text-property (point)
                                 'vtable--row-color-face-record)))))
  (defun remacs-vtable3030-distinct-row-tokens-p (rows)
    (let ((tokens (mapcar (lambda (row) (nth 7 row)) rows)))
      (and (cl-every #'identity tokens)
           (cl-loop for tail on tokens
                    always (not (memq (car tail) (cdr tail)))))))
  (defun remacs-vtable3030-test-insert-front-row-colors ()
    (with-temp-buffer
      (setq remacs-vtable3030-getter-calls 0
            remacs-vtable3030-formatter-calls 0
            remacs-vtable3030-displayer-calls 0)
      (let* ((a (vector "a"))
             (b (vector "b"))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list a b)
                                 :use-header-line nil
                                 :row-colors '("red" "blue")
                                 :getter #'remacs-vtable3030-getter
                                 :formatter #'remacs-vtable3030-formatter
                                 :displayer #'remacs-vtable3030-displayer))
             (columns (vtable-columns table))
             (sort-before (vtable-sort-by table))
             (cache-key (slot-value table '-current-cache-key))
             (cache (vtable--current-cache table))
             (cache-table (slot-value table '-cache))
             (other-key (cons (car cache-key) (1+ (cdr cache-key))))
             (before (remacs-vtable3030-row-snapshot table))
             (calls-before remacs-vtable3030-getter-calls)
             (format-before remacs-vtable3030-formatter-calls)
             (display-before remacs-vtable3030-displayer-calls))
        (puthash other-key (copy-tree cache) cache-table)
        (setq buffer-read-only t)
        (let ((result (vtable-insert-object table new 0)))
          (let ((rows (remacs-vtable3030-row-snapshot table)))
            (remacs-vtable3030-assert (null result)
                                      "Insert return value changed")
            (remacs-vtable3030-assert
             (and (= (length rows) 3)
                  (eq (car (nth 0 rows)) new)
                  (eq (car (nth 1 rows)) a)
                  (eq (car (nth 2 rows)) b))
             "Front insertion changed object order or identity")
            (remacs-vtable3030-check-row-color
             (nth 0 rows) '(:background "red"))
            (remacs-vtable3030-check-row-color
             (nth 1 rows) '(:background "blue"))
            (remacs-vtable3030-check-row-color
             (nth 2 rows) '(:background "red"))
            (remacs-vtable3030-assert
             (and (vectorp (cadddr (nth 0 rows)))
                  (vectorp (cadddr (nth 1 rows)))
                  (vectorp (cadddr (nth 2 rows))))
             "Row-color base-face markers were not stored")
            (remacs-vtable3030-assert
             (and (equal (nth 2 (nth 0 before)) (nth 2 (nth 1 rows)))
                  (equal (nth 2 (nth 1 before)) (nth 2 (nth 2 rows))))
             "Color refresh changed surviving row text"))
          (remacs-vtable3030-assert
           (equal (vtable-objects table) (list new a b))
           "Object storage did not match the inserted rows")
          (remacs-vtable3030-assert
           (eq columns (vtable-columns table))
           "Insert replaced the column list")
          (remacs-vtable3030-assert
           (equal sort-before (vtable-sort-by table))
           "Insert changed sort settings")
          (remacs-vtable3030-assert
           (and (eq cache (vtable--current-cache table))
                (eq cache-key (slot-value table '-current-cache-key))
                (eq cache (gethash cache-key cache-table))
                (null (gethash other-key cache-table)))
           "Insert changed the active cache or retained another cache")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-getter-calls (1+ calls-before))
           "Color refresh re-ran getters beyond the inserted row")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-formatter-calls (1+ format-before))
           "Color-only refresh re-ran existing formatters")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-displayer-calls (1+ display-before))
           "Color-only refresh re-ran existing displayers")
          (remacs-vtable3030-assert
           (eq buffer-read-only t) "Insert changed read-only state"))
        t)))
  (defun remacs-vtable3030-test-remove-duplicate-rows ()
    (with-temp-buffer
      (setq remacs-vtable3030-getter-calls 0
            remacs-vtable3030-formatter-calls 0
            remacs-vtable3030-displayer-calls 0)
      (let* ((same (vector "same"))
             (equal-but-distinct (vector "same"))
             (tail (vector "tail"))
             (table (make-vtable :columns ["Name"]
                                 :objects (list same same equal-but-distinct
                                                same tail)
                                 :use-header-line nil
                                 :row-colors '("red" "blue" "green")
                                 :getter #'remacs-vtable3030-getter
                                 :formatter #'remacs-vtable3030-formatter
                                 :displayer #'remacs-vtable3030-displayer))
             (columns (vtable-columns table))
             (sort-before (vtable-sort-by table))
             (cache-key (slot-value table '-current-cache-key))
             (cache (vtable--current-cache table))
             (cache-table (slot-value table '-cache))
             (other-key (cons (car cache-key) (1+ (cdr cache-key))))
             (calls-before remacs-vtable3030-getter-calls)
             (format-before remacs-vtable3030-formatter-calls)
             (display-before remacs-vtable3030-displayer-calls))
        (remacs-vtable3030-assert
         (and (equal same equal-but-distinct)
              (not (eq same equal-but-distinct)))
         "Duplicate fixture must contain equal but distinct objects")
        (let ((logical-rows (remacs-vtable3030-logical-row-snapshot table)))
          (remacs-vtable3030-assert
           (and (= (length logical-rows) 5)
                (eq (car (nth 0 logical-rows)) same)
                (eq (car (nth 1 logical-rows)) same)
                (remacs-vtable3030-distinct-row-tokens-p logical-rows))
           "Logical row tokens merged repeated EQ objects"))
        (puthash other-key (copy-tree cache) cache-table)
        (setq buffer-read-only t)
        (let* ((before (remacs-vtable3030-row-snapshot table))
               (result (vtable-remove-object table same))
               (rows (remacs-vtable3030-row-snapshot table))
               (cache-lines (vtable--cache-lines cache)))
          (remacs-vtable3030-assert
           (and (= (length before) 5)
                (eq (car (nth 0 before)) same)
                (eq (car (nth 1 before)) same)
                (eq (car (nth 2 before)) equal-but-distinct)
                (eq (car (nth 3 before)) same))
           "Adjacent/nonadjacent duplicate rows were not present")
          (remacs-vtable3030-assert (null result)
                                    "Remove return value changed")
          (remacs-vtable3030-assert
           (and (= (length rows) 2)
                (eq (car (nth 0 rows)) equal-but-distinct)
                (eq (car (nth 1 rows)) tail))
           "Remove changed surviving row identity or order")
          (remacs-vtable3030-check-row-color
           (nth 0 rows) '(:background "red"))
          (remacs-vtable3030-check-row-color
           (nth 1 rows) '(:background "blue"))
          (remacs-vtable3030-assert
           (and (vectorp (cadddr (nth 0 rows)))
                (vectorp (cadddr (nth 1 rows))))
           "Remaining row-color base-face markers were lost")
          (remacs-vtable3030-assert
           (and (equal (nth 2 (nth 2 before)) (nth 2 (nth 0 rows)))
                (equal (nth 2 (nth 4 before)) (nth 2 (nth 1 rows))))
           "Removal/color refresh changed surviving row text")
          (remacs-vtable3030-assert
           (and (equal (vtable-objects table) (list equal-but-distinct tail))
                (= (length cache-lines) 2)
                (eq (car (nth 0 cache-lines)) equal-but-distinct)
                (eq (car (nth 1 cache-lines)) tail))
           "Object storage/cache did not match the surviving rows")
          (remacs-vtable3030-assert
           (eq columns (vtable-columns table))
           "Remove replaced the column list")
          (remacs-vtable3030-assert
           (equal sort-before (vtable-sort-by table))
           "Remove changed sort settings")
          (remacs-vtable3030-assert
           (and (eq cache (vtable--current-cache table))
                (eq cache-key (slot-value table '-current-cache-key))
                (eq cache (gethash cache-key cache-table))
                (null (gethash other-key cache-table)))
           "Remove changed the active cache or retained another cache")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-getter-calls calls-before)
           "Remove/color refresh re-ran getters")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-formatter-calls format-before)
           "Color-only remove re-ran formatters")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-displayer-calls display-before)
           "Color-only remove re-ran displayers")
          (remacs-vtable3030-assert
           (eq buffer-read-only t) "Remove changed read-only state"))
        t)))
  (defun remacs-vtable3030-test-face-expressions ()
    (with-temp-buffer
      (let* ((a (vector "alpha"))
             (b (vector "beta"))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list a b)
                                 :use-header-line nil :face 'bold
                                 :row-colors '(bold (:foreground "magenta")))))
        ;; Preserve user-added face and display text properties through a
        ;; structural row-color update.
        (vtable-goto-table table)
        (forward-line 1)
        (vtable-goto-object a)
        (vtable-goto-column 0)
        (forward-char 1)
        (put-text-property (point) (1+ (point)) 'display '(space :width 2))
        (add-face-text-property (point) (1+ (point)) 'underline)
        (vtable-insert-object table new 0)
        (let ((rows (remacs-vtable3030-row-snapshot table 0)))
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p (cadr (nth 0 rows)) 'bold)
                (remacs-vtable3030-face-has-p
                 (aref (cadddr (nth 0 rows)) 0) 'bold))
           "Same row/base face expression was lost")
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p
                 (cadr (nth 1 rows)) '(:foreground "magenta"))
                (remacs-vtable3030-face-has-p (cadr (nth 1 rows)) 'bold)
                (not (remacs-vtable3030-face-has-p
                      (cadr (nth 1 rows)) 'underline))
                (remacs-vtable3030-face-has-p
                 (nth 5 (nth 1 rows)) 'underline)
                (remacs-vtable3030-face-has-p
                 (aref (cadddr (nth 1 rows)) 0) 'bold)
                (null (nth 4 (nth 1 rows)))
                (equal (nth 6 (nth 1 rows)) '(space :width 2)))
           "Face-spec row color or user face/display property was lost")
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p (cadr (nth 2 rows)) 'bold)
                (zerop (remacs-vtable3030-face-count
                        (cadr (nth 2 rows)) '(:foreground "magenta")))
                (remacs-vtable3030-face-has-p
                 (aref (cadddr (nth 2 rows)) 0) 'bold))
           "Repeated row/base face expression was lost")
          (vtable-remove-object table new)
          (let ((rows (remacs-vtable3030-row-snapshot table 0)))
            (remacs-vtable3030-assert
             (and (= (length rows) 2)
                  (eq (car (nth 0 rows)) a)
                  (eq (car (nth 1 rows)) b)
                  (remacs-vtable3030-face-has-p
                   (cadr (nth 0 rows)) 'bold)
                  (remacs-vtable3030-face-has-p
                   (cadr (nth 1 rows)) '(:foreground "magenta"))
                  (= (remacs-vtable3030-face-count
                      (cadr (nth 1 rows)) 'bold) 1)
                  (remacs-vtable3030-face-has-p
                   (nth 5 (nth 0 rows)) 'underline)
                  (equal (nth 6 (nth 0 rows)) '(space :width 2)))
             "Moving a row color equal to the base face left a stale face or lost user style")))))
    ;; Equal palette entries and a matching user face must not accumulate
    ;; stale face specs when rows move more than once.
    (with-temp-buffer
      (let* ((a (vector "a"))
             (b (vector "b"))
             (new (vector "new"))
             (color '(:foreground "magenta"))
             (table (make-vtable :columns ["Name"] :objects (list a b)
                                 :use-header-line nil :face 'shadow
                                 :row-colors (list color color))))
        (vtable-insert-object table new 0)
        (vtable-remove-object table a)
        (let ((rows (remacs-vtable3030-row-snapshot table 0)))
          (remacs-vtable3030-assert
           (and (= (length rows) 2)
                (eq (car (nth 0 rows)) new)
                (eq (car (nth 1 rows)) b))
           "Duplicate face-palette refresh changed row identities")
          (dolist (row rows)
            (remacs-vtable3030-assert
             (= (remacs-vtable3030-face-count (cadr row) color) 1)
             "Repeated palette face accumulated duplicate properties")
            (remacs-vtable3030-assert
             (remacs-vtable3030-face-has-p
              (aref (cadddr row) 0) 'shadow)
             "Duplicate face-palette refresh lost the base face")))))
    t)
  (defun remacs-vtable3030-test-column-color-blending ()
    (with-temp-buffer
      (setq remacs-vtable3030-getter-calls 0
            remacs-vtable3030-formatter-calls 0
            remacs-vtable3030-displayer-calls 0)
      (let* ((a (vector "a" "old"))
             (b (vector "b" "old"))
             (new (vector "new" "new"))
             (table (make-vtable :columns ["Name" "Value"]
                                 :objects (list a b)
                                 :use-header-line nil
                                 :row-colors '("#ff0000" "#0000ff")
                                 :column-colors '("#00ff00" "#ffffff")
                                 :divider "|"
                                 :getter #'remacs-vtable3030-getter
                                 :formatter #'remacs-vtable3030-formatter
                                 :displayer #'remacs-vtable3030-displayer))
             (cache (vtable--current-cache table))
             (cache-key (slot-value table '-current-cache-key))
             (columns (vtable-columns table))
             (before0 (remacs-vtable3030-row-snapshot table 0))
             (before1 (remacs-vtable3030-row-snapshot table 1))
             (calls-before remacs-vtable3030-getter-calls)
             (format-before remacs-vtable3030-formatter-calls)
             (display-before remacs-vtable3030-displayer-calls))
        (setq buffer-read-only t)
        (let ((insert-result (vtable-insert-object table new 0)))
          (let ((rows0 (remacs-vtable3030-row-snapshot table 0))
                (rows1 (remacs-vtable3030-row-snapshot table 1)))
            (remacs-vtable3030-assert (null insert-result)
                                      "Blended-color insert return changed")
            (remacs-vtable3030-assert
             (and (eq (car (nth 0 rows0)) new)
                  (eq (car (nth 1 rows0)) a)
                  (eq (car (nth 2 rows0)) b))
             "Blended-color insert changed row identity/order")
            (remacs-vtable3030-check-blend-colors table rows0 0)
            (remacs-vtable3030-check-blend-colors table rows1 1)
            (remacs-vtable3030-assert
             (and (equal (nth 2 (nth 0 before0)) (nth 2 (nth 1 rows0)))
                  (equal (nth 2 (nth 1 before0)) (nth 2 (nth 2 rows0)))
                  (equal (nth 2 (nth 0 before1)) (nth 2 (nth 1 rows1)))
                  (equal (nth 2 (nth 1 before1)) (nth 2 (nth 2 rows1))))
             "Color-only insert changed existing cell text")
            (remacs-vtable3030-assert
             (and (vectorp (cadddr (nth 0 rows0)))
                  (vectorp (cadddr (nth 1 rows0)))
                  (vectorp (cadddr (nth 2 rows0)))
                  (vectorp (cadddr (nth 0 rows1)))
                  (vectorp (cadddr (nth 1 rows1)))
                  (vectorp (cadddr (nth 2 rows1))))
             "Column-color base-face markers were not stored"))
          (remacs-vtable3030-assert
           (= remacs-vtable3030-getter-calls (+ calls-before 2))
           "Blended-color refresh re-ran getters beyond the new row")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-formatter-calls (+ format-before 2))
           "Blended-color refresh re-ran existing formatters")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-displayer-calls (+ display-before 2))
           "Blended-color refresh re-ran existing displayers"))
        (let ((remove-result (vtable-remove-object table a)))
          (let ((rows0 (remacs-vtable3030-row-snapshot table 0))
                (rows1 (remacs-vtable3030-row-snapshot table 1)))
            (remacs-vtable3030-assert (null remove-result)
                                      "Blended-color remove return changed")
            (remacs-vtable3030-assert
             (and (= (length rows0) 2)
                  (eq (car (nth 0 rows0)) new)
                  (eq (car (nth 1 rows0)) b))
             "Blended-color remove changed row identity/order")
            (remacs-vtable3030-check-blend-colors table rows0 0)
            (remacs-vtable3030-check-blend-colors table rows1 1)
            (remacs-vtable3030-assert
             (and (equal (nth 2 (nth 1 before0)) (nth 2 (nth 1 rows0)))
                  (equal (nth 2 (nth 1 before1)) (nth 2 (nth 1 rows1))))
             "Color-only remove changed surviving cell text"))
          (remacs-vtable3030-assert
           (= remacs-vtable3030-getter-calls (+ calls-before 2))
           "Blended-color remove re-ran getters")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-formatter-calls (+ format-before 2))
           "Blended-color remove re-ran formatters")
          (remacs-vtable3030-assert
           (= remacs-vtable3030-displayer-calls (+ display-before 2))
           "Blended-color remove re-ran displayers"))
        (remacs-vtable3030-assert
         (and (eq cache (vtable--current-cache table))
              (eq cache-key (slot-value table '-current-cache-key))
              (eq columns (vtable-columns table)))
         "Blended-color updates changed cache key/cache/columns")
        (remacs-vtable3030-assert
         (eq buffer-read-only t) "Blended-color updates changed read-only state")
        t)))
  (defun remacs-vtable3030-test-header-modes ()
    ;; Auto-generated columns have no in-buffer header.
    (with-temp-buffer
      (let* ((a (vector "a"))
             (b (vector "b"))
             (new (vector "new"))
             (table (make-vtable :objects (list a b)
                                 :use-header-line nil
                                 :row-colors '("red" "blue"))))
        (remacs-vtable3030-assert
         (not (slot-value table '-has-column-spec))
         "No-header fixture unexpectedly has a column specification")
        (vtable-insert-object table new 0)
        (let ((rows (remacs-vtable3030-row-snapshot table)))
          (remacs-vtable3030-assert
           (and (= (length rows) 3)
                (eq (car (nth 0 rows)) new)
                (eq (car (nth 1 rows)) a)
                (eq (car (nth 2 rows)) b))
           "No-header insertion used a header offset")
          (remacs-vtable3030-check-row-color
           (nth 0 rows) '(:background "red"))
          (remacs-vtable3030-check-row-color
           (nth 1 rows) '(:background "blue"))
          (remacs-vtable3030-check-row-color
           (nth 2 rows) '(:background "red")))
        t))
    ;; Header-line mode must not advance through a nonexistent buffer line.
    (with-temp-buffer
      (let* ((a (vector "a"))
             (b (vector "b"))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list a b)
                                 :row-colors '("red" "blue")))
             (header-line header-line-format))
        (remacs-vtable3030-assert header-line
                                  "Header-line fixture has no header")
        (vtable-insert-object table new 0)
        (let ((rows (remacs-vtable3030-row-snapshot table)))
          (remacs-vtable3030-assert
           (and (= (length rows) 3)
                (eq (car (nth 0 rows)) new)
                (eq (car (nth 1 rows)) a)
                (eq (car (nth 2 rows)) b))
           "Header-line insertion changed data-row positions")
          (remacs-vtable3030-check-row-color
           (nth 0 rows) '(:background "red"))
          (remacs-vtable3030-check-row-color
           (nth 1 rows) '(:background "blue"))
          (remacs-vtable3030-check-row-color
           (nth 2 rows) '(:background "red")))
        (remacs-vtable3030-assert
         (eq header-line-format header-line)
         "Structural update replaced header-line contents")
        (vtable-remove-object table new)
        (remacs-vtable3030-assert
         (eq header-line-format header-line)
         "Removing a row replaced header-line contents")
        t)))
  (defun remacs-vtable3030-test-sort-update-revert ()
    (with-temp-buffer
      (setq remacs-vtable3030-getter-calls 0)
      (let* ((b (vector "b"))
             (a (vector "a"))
             (updated-b (vector "c"))
             (table (make-vtable :columns ["Name"] :objects (list b a)
                                 :use-header-line nil
                                 :row-colors '("red" "blue")
                                 :getter #'remacs-vtable3030-getter))
             (cache (vtable--current-cache table))
             (cache-key (slot-value table '-current-cache-key))
             (columns (vtable-columns table))
             (calls-before remacs-vtable3030-getter-calls))
        (setq buffer-read-only t)
        (vtable-goto-table table)
        (forward-line 1)
        (vtable-goto-column 0)
        (vtable-sort-by-current-column)
        (let ((rows (remacs-vtable3030-row-snapshot table)))
          (remacs-vtable3030-assert
           (and (eq (car (nth 0 rows)) a) (eq (car (nth 1 rows)) b))
           "Sort did not order rows")
          (remacs-vtable3030-check-row-color
           (nth 0 rows) '(:background "red"))
          (remacs-vtable3030-check-row-color
           (nth 1 rows) '(:background "blue")))
        (remacs-vtable3030-assert
         (= remacs-vtable3030-getter-calls calls-before)
         "Sort re-ran getters")
        (vtable-update-object table updated-b b)
        (let ((rows (remacs-vtable3030-row-snapshot table)))
          (remacs-vtable3030-assert
           (and (eq (car (nth 0 rows)) a)
                (eq (car (nth 1 rows)) updated-b))
           "Update changed row order or object identity")
          (remacs-vtable3030-check-row-color
           (nth 0 rows) '(:background "red"))
          (remacs-vtable3030-check-row-color
           (nth 1 rows) '(:background "blue")))
        (remacs-vtable3030-assert
         (= remacs-vtable3030-getter-calls (1+ calls-before))
         "Update getter count changed beyond the updated row")
        (vtable-revert table)
        (let ((rows (remacs-vtable3030-row-snapshot table)))
          (remacs-vtable3030-assert
           (and (eq (car (nth 0 rows)) a)
                (eq (car (nth 1 rows)) updated-b))
           "Revert changed row order or object identity")
          (remacs-vtable3030-check-row-color
           (nth 0 rows) '(:background "red"))
          (remacs-vtable3030-check-row-color
           (nth 1 rows) '(:background "blue")))
        (remacs-vtable3030-assert
         (= remacs-vtable3030-getter-calls (1+ calls-before))
         "Cache-backed revert re-ran getters")
        (remacs-vtable3030-assert
         (and (eq cache (vtable--current-cache table))
              (equal cache-key (slot-value table '-current-cache-key))
              (eq columns (vtable-columns table))
              (equal (vtable-sort-by table) '((0 . ascend)))
              (eq buffer-read-only t))
         "Sort/update/revert changed cache, columns, sort, or read-only state")
        t)))
  (defun remacs-vtable3030-test-multiline-row-spans ()
    (with-temp-buffer
      (setq remacs-vtable3030-getter-calls 0
            remacs-vtable3030-formatter-calls 0
            remacs-vtable3030-displayer-calls 0)
      (let* ((a (vector "A"))
             (b (vector "B-top
B-bottom"))
             (c (vector "C"))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list a b c)
                                 :use-header-line nil
                                 :row-colors '("red" "blue")
                                 :getter #'remacs-vtable3030-getter))
             (before (remacs-vtable3030-logical-row-snapshot table))
             (getter-before remacs-vtable3030-getter-calls))
        ;; The one-line A row before B keeps the existing insertion anchor path
        ;; independent of this test's multi-line recoloring case.
        (setq buffer-read-only t)
        (let ((result (vtable-insert-object table new 0)))
          (let ((rows (remacs-vtable3030-logical-row-snapshot table)))
            (remacs-vtable3030-assert (null result)
                                      "Multi-line insert return value changed")
            (remacs-vtable3030-assert
             (and (= (length rows) 4)
                  (eq (car (nth 0 rows)) new)
                  (eq (car (nth 1 rows)) a)
                  (eq (car (nth 2 rows)) b)
                  (eq (car (nth 3 rows)) c))
             "Multi-line insert lost logical row order")
            (remacs-vtable3030-check-row-color
             (nth 0 rows) '(:background "red"))
            (remacs-vtable3030-check-row-color
             (nth 1 rows) '(:background "blue"))
            (remacs-vtable3030-check-row-color
             (nth 2 rows) '(:background "red"))
            (remacs-vtable3030-check-row-color
             (nth 3 rows) '(:background "blue"))
            (remacs-vtable3030-assert
             (and (equal (nth 2 (nth 0 before)) (nth 2 (nth 1 rows)))
                  (equal (nth 2 (nth 1 before)) (nth 2 (nth 2 rows)))
                  (equal (nth 2 (nth 2 before)) (nth 2 (nth 3 rows)))
                  (string-match-p "B-top\nB-bottom" (nth 2 (nth 2 rows)))
                  (vectorp (cadddr (nth 2 rows))))
             "Multi-line insert changed cell text or row token span")))
        (remacs-vtable3030-check-logical-row-span-face
         table b '(:background "red"))
        (remacs-vtable3030-check-logical-row-span-face
         table c '(:background "blue"))
        (remacs-vtable3030-assert
         (= remacs-vtable3030-getter-calls (1+ getter-before))
         "Multi-line color update re-ran existing getters")
        (let ((result (vtable-remove-object table a)))
          (let ((rows (remacs-vtable3030-logical-row-snapshot table)))
            (remacs-vtable3030-assert (null result)
                                      "Multi-line remove return value changed")
            (remacs-vtable3030-assert
             (and (= (length rows) 3)
                  (eq (car (nth 0 rows)) new)
                  (eq (car (nth 1 rows)) b)
                  (eq (car (nth 2 rows)) c))
             "Multi-line remove lost logical row order")
            (remacs-vtable3030-check-row-color
             (nth 0 rows) '(:background "red"))
            (remacs-vtable3030-check-row-color
             (nth 1 rows) '(:background "blue"))
            (remacs-vtable3030-check-row-color
             (nth 2 rows) '(:background "red"))
            (remacs-vtable3030-assert
             (and (equal (nth 2 (nth 1 before)) (nth 2 (nth 1 rows)))
                  (string-match-p "B-top\nB-bottom" (nth 2 (nth 1 rows))))
             "Multi-line remove changed surviving row text")))
        (remacs-vtable3030-check-logical-row-span-face
         table b '(:background "blue"))
        (remacs-vtable3030-check-logical-row-span-face
         table c '(:background "red"))
        (remacs-vtable3030-assert
         (= remacs-vtable3030-getter-calls (1+ getter-before))
         "Multi-line remove re-ran getters")
        (remacs-vtable3030-assert
         (eq buffer-read-only t) "Multi-line updates changed read-only state")
        t)))
  (defun remacs-vtable3030-test-legacy-face-conses ()
    (dolist (legacy-base '((foreground-color . "green")
                          (background-color . "white")))
     (with-temp-buffer
      (let* ((foreground-face '(foreground-color . "green"))
             (background-face '(background-color . "white"))
             (a-text "alpha")
             (b-text "beta")
             (a (vector a-text))
             (b (vector b-text))
             (new (vector "new"))
             (table (make-vtable :columns ["Name"] :objects (list a b)
                                 :face legacy-base :use-header-line nil
                                 :row-colors '("red" "blue"))))
        (remacs-vtable3030-assert
         (equal (vtable--face-components foreground-face)
                (list foreground-face))
         "Legacy foreground face was not normalized as one component")
        (remacs-vtable3030-assert
         (equal (vtable--face-components background-face)
                (list background-face))
         "Legacy background face was not normalized as one component")
        (vtable-goto-object a)
        (vtable-goto-column 0)
        (forward-char 1)
        (put-text-property (point) (1+ (point)) 'face
                           '(background-color . "yellow"))
        (vtable-goto-object b)
        (vtable-goto-column 0)
        (forward-char 1)
        (put-text-property (point) (1+ (point)) 'face
                           '(foreground-color . "cyan"))
        (vtable-insert-object table new 0)
        (let* ((a-state (remacs-vtable3030-cell-face-state table a 1))
               (b-state (remacs-vtable3030-cell-face-state table b 1))
               (a-record (cadr a-state))
               (b-record (cadr b-state)))
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p
                 (car a-state) '(:background "blue"))
                (remacs-vtable3030-face-has-p
                 (car a-state) '(background-color . "yellow"))
                (vectorp a-record)
                (equal (aref a-record 0) '(background-color . "yellow"))
                (= (remacs-vtable3030-face-count
                    (car a-state) '(:background "blue")) 1))
           "Legacy foreground base lost a user-edited background face")
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p
                 (car b-state) '(:background "red"))
                (remacs-vtable3030-face-has-p
                 (car b-state) '(foreground-color . "cyan"))
                (vectorp b-record)
                (equal (aref b-record 0) '(foreground-color . "cyan"))
                (= (remacs-vtable3030-face-count
                    (car b-state) '(:background "red")) 1))
           "Legacy background base lost a user-edited foreground face"))
        (vtable-remove-object table new)
        (let ((a-state (remacs-vtable3030-cell-face-state table a 1))
              (b-state (remacs-vtable3030-cell-face-state table b 1)))
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p
                 (car a-state) '(:background "red"))
                (remacs-vtable3030-face-has-p
                 (car a-state) '(background-color . "yellow")))
           "Second legacy-face recolor lost the user background")
          (remacs-vtable3030-assert
           (and (remacs-vtable3030-face-has-p
                 (car b-state) '(:background "blue"))
                (remacs-vtable3030-face-has-p
                 (car b-state) '(foreground-color . "cyan")))
           "Second legacy-face recolor lost the user foreground"))
        t)))
    t)
  ;; Call each scenario independently so a failed result cannot be masked by
  ;; another successful helper or a final truth value.
  (unless (remacs-vtable3030-test-insert-front-row-colors)
    (error "Front-insert color scenario failed"))
  (unless (remacs-vtable3030-test-remove-duplicate-rows)
    (error "Duplicate-remove color scenario failed"))
  (unless (remacs-vtable3030-test-face-expressions)
    (error "Face-expression color scenario failed"))
  (unless (remacs-vtable3030-test-column-color-blending)
    (error "Column-color blending scenario failed"))
  (unless (remacs-vtable3030-test-header-modes)
    (error "Header-mode color scenario failed"))
  (unless (remacs-vtable3030-test-sort-update-revert)
    (error "Sort/update/revert color scenario failed"))
  (unless (remacs-vtable3030-test-multiline-row-spans)
    (error "Multi-line row-span color scenario failed"))
  (unless (remacs-vtable3030-test-legacy-face-conses)
    (error "Legacy face-cons color scenario failed"))
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
fn vtable_structural_changes_recolor_cached_rows_3030() {
    assert_eq!(run_with_library("lisp/vtable.el"), "t");
    assert_eq!(run_with_library("lisp-elc/vtable.elc"), "t");
}
