;;; vtable.el --- Displaying data in tables  -*- lexical-binding: t; -*-

;; Copyright (C) 2022-2026 Free Software Foundation, Inc.

;; This file is part of GNU Emacs.

;; GNU Emacs is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; GNU Emacs is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with GNU Emacs.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;;

;;; Code:

(require 'cl-lib)
(require 'eieio)
(require 'text-property-search)
(require 'mule-util)

(defface vtable
  '((t :inherit variable-pitch))
  "Face used (by default) for vtables."
  :version "29.1"
  :group 'faces)

(cl-defstruct vtable-column
  "A vtable column."
  name
  width
  min-width
  max-width
  primary
  align
  getter
  formatter
  displayer
  -numerical
  -aligned)

(defclass vtable ()
  ((columns :initarg :columns :accessor vtable-columns)
   (objects :initarg :objects :accessor vtable-objects)
   (objects-function :initarg :objects-function
                     :accessor vtable-objects-function)
   (getter :initarg :getter :accessor vtable-getter)
   (formatter :initarg :formatter :accessor vtable-formatter)
   (displayer :initarg :displayer :accessor vtable-displayer)
   (use-header-line :initarg :use-header-line
                    :accessor vtable-use-header-line)
   (face :initarg :face :accessor vtable-face)
   (actions :initarg :actions :accessor vtable-actions)
   (keymap :initarg :keymap :accessor vtable-keymap)
   (separator-width :initarg :separator-width :accessor vtable-separator-width)
   (divider :initarg :divider :accessor vtable-divider :initform nil)
   (sort-by :initarg :sort-by :accessor vtable-sort-by)
   (ellipsis :initarg :ellipsis :accessor vtable-ellipsis)
   (column-colors :initarg :column-colors :accessor vtable-column-colors)
   (row-colors :initarg :row-colors :accessor vtable-row-colors)
   (buffer :initform nil :accessor vtable-buffer)
   (-cached-colors :initform nil)
   (-cache :initform (make-hash-table :test #'equal))
   (-current-cache :initform nil :accessor vtable--current-cache)
   (-current-cache-key :initform nil)
   (-start-marker :initform nil)
   (-cached-keymap :initform nil)
   (-has-column-spec :initform nil))
  "An object to hold the data for a table.")

(defvar-keymap vtable-map
  "S" #'vtable-sort-by-current-column
  "{" #'vtable-narrow-current-column
  "}" #'vtable-widen-current-column
  "g" #'vtable-revert-command
  "M-<left>" #'vtable-previous-column
  "M-<right>" #'vtable-next-column)

(defvar-keymap vtable-header-line-map
  :parent vtable-map
  "<follow-link>" 'mouse-face
  "<mouse-2>" #'vtable-header-line-sort)

(cl-defun make-vtable (&key columns objects objects-function
                            getter
                            formatter
                            displayer
                            (use-header-line t)
                            (face 'vtable)
                            actions keymap
                            (separator-width 1)
                            divider
                            divider-width
                            sort-by
                            (ellipsis t)
                            (insert t)
                            row-colors
                            column-colors)
  "Create and insert a vtable at point.
The vtable object is returned.  If INSERT is nil, the table won't
be inserted.

See info node `(vtable)Top' for vtable documentation."
  (when objects-function
    (setq objects (funcall objects-function)))
  ;; We'll be altering the list, so create a copy.
  (setq objects (copy-sequence objects))
  (let ((table
         (make-instance
          'vtable
          :objects objects
          :objects-function objects-function
          :getter getter
          :formatter formatter
          :displayer displayer
          :use-header-line use-header-line
          :face face
          :actions actions
          :keymap keymap
          :separator-width separator-width
          :sort-by sort-by
          :row-colors row-colors
          :column-colors column-colors
          :ellipsis ellipsis)))
    ;; Store whether the user has specified columns or not.
    (setf (slot-value table '-has-column-spec) (not (not columns)))
    ;; Auto-generate the columns.
    (unless columns
      (unless objects
        (error "Can't auto-generate columns; no objects"))
      (setq columns (make-list (length (car objects)) "")))
    (setf (vtable-columns table)
          (mapcar (lambda (column)
                    (cond
                     ;; We just have the name (as a string).
                     ((stringp column)
                      (make-vtable-column :name column))
                     ;; A plist of keywords/values.
                     ((listp column)
                      (apply #'make-vtable-column column))
                     ;; A full `vtable-column' object.
                     (t
                      column)))
                  columns))
    ;; Compute missing column data.
    (setf (vtable-columns table) (vtable--compute-columns table))
    ;; Compute the colors.
    (when (or row-colors column-colors)
      (setf (slot-value table '-cached-colors)
            (vtable--compute-colors row-colors column-colors)))
    ;; Compute the divider.
    (when (or divider divider-width)
      (setf (vtable-divider table)
            (propertize
             (or (copy-sequence divider)
                 (propertize
                  " " 'display
                  (list 'space :width
                        (list (vtable--compute-width table divider-width)))))
             'mouse-face 'highlight
             'keymap
             (define-keymap
               "<drag-mouse-1>" #'vtable--drag-resize-column
               "<down-mouse-1>" #'ignore))))
    ;; Compute the keymap.
    (setf (slot-value table '-cached-keymap) (vtable--make-keymap table))
    (unless sort-by
      (seq-do-indexed (lambda (column index)
                        (when (vtable-column-primary column)
                          (push (cons index (vtable-column-primary column))
                                (vtable-sort-by table))))
                      (vtable-columns table)))
    (when insert
      (vtable-insert table))
    table))

(defun vtable--compute-colors (row-colors column-colors)
  (cond
   ((null column-colors)
    (mapcar #'vtable--make-color-face row-colors))
   ((null row-colors)
    (mapcar #'vtable--make-color-face column-colors))
   (t
    (cl-loop for row in row-colors
             collect (cl-loop for column in column-colors
                              collect (vtable--face-blend
                                       (vtable--make-color-face row)
                                       (vtable--make-color-face column)))))))

(defun vtable--make-color-face (object)
  (if (stringp object)
      (list :background object)
    object))

(defun vtable--face-blend (face1 face2)
  (let ((foreground (vtable--face-color face1 face2 #'face-foreground
                                        :foreground))
        (background (vtable--face-color face1 face2 #'face-background
                                        :background)))
    `(,@(and foreground (list :foreground foreground))
      ,@(and background (list :background background)))))

(defun vtable--face-color (face1 face2 accessor slot)
  (let ((col1 (if (facep face1)
                  (funcall accessor face1)
                (plist-get face1 slot)))
        (col2 (if (facep face2)
                  (funcall accessor face2)
                (plist-get face2 slot))))
    (if (and col1 col2)
        (apply #'color-rgb-to-hex
               `(,@(color-blend (color-name-to-rgb col1)
                                (color-name-to-rgb col2))
                 2))
      (or col1 col2))))

;;; Interface utility functions.

(defun vtable-current-table ()
  "Return the table under point."
  (get-text-property (point) 'vtable))

(defun vtable-current-object ()
  "Return the object under point."
  (get-text-property (point) 'vtable-object))

(defun vtable-current-column ()
  "Return the index of the column under point."
  (get-text-property (point) 'vtable-column))

(defun vtable--start-marker-position (table)
  "Return TABLE's accessible start marker position in the current buffer."
  (let* ((marker (slot-value table '-start-marker))
         (position (and (markerp marker)
                        (eq (marker-buffer marker) (current-buffer))
                        (marker-position marker))))
    (and position
         (<= (point-min) position)
         (<= position (point-max))
         position)))

(defun vtable--set-start-marker (table position &optional buffer)
  "Set TABLE's start marker to POSITION in BUFFER, preserving its identity."
  (let ((marker (slot-value table '-start-marker)))
    (unless (markerp marker)
      (setq marker (make-marker))
      (setf (slot-value table '-start-marker) marker))
    (if position
        (set-marker marker position (or buffer (current-buffer)))
      (set-marker marker nil))
    (set-marker-insertion-type marker t)
    marker))

(defun vtable--beginning-of-table (table)
  (if (or (text-property-search-backward 'vtable table #'eq)
          (get-text-property (point) 'vtable))
      (point)
    (if table
        (if-let* ((marker (vtable--start-marker-position table)))
            (goto-char marker)
          (user-error "No table insertion boundary"))
      (goto-char (point-min)))))

(defun vtable-beginning-of-table ()
  "Go to the start of the current table."
  (vtable--beginning-of-table (vtable-current-table)))

(defun vtable--end-of-table (table)
  (if (or (text-property-search-forward 'vtable table #'eq)
          (get-text-property (point) 'vtable))
      (point)
    (if table
        (if-let* ((marker (vtable--start-marker-position table)))
            (goto-char marker)
          (user-error "No table insertion boundary"))
      (goto-char (point-max)))))

(defun vtable-end-of-table ()
  "Go to the end of the current table."
  (vtable--end-of-table (vtable-current-table)))

(defun vtable-goto-object (object)
  "Go to OBJECT in the current table.
Return the position of the object if found, and nil if not."
  (let ((start (point)))
    (vtable-beginning-of-table)
    (save-restriction
      (narrow-to-region (point) (save-excursion (vtable-end-of-table)))
      (if (text-property-search-forward 'vtable-object object #'eq)
          (progn
            (forward-line -1)
            (point))
        (goto-char start)
        nil))))

(defun vtable-goto-table (table)
  "Go to TABLE in the current buffer.
If TABLE is found, return the position of the start of the table.
For a zero-width, empty table, use its recorded insertion marker.
If TABLE can't be found, return nil and don't move point."
  (let ((start (point)))
    (goto-char (point-min))
    (if-let* ((match (text-property-search-forward 'vtable table t)))
        (goto-char (prop-match-beginning match))
      (if-let* ((marker (vtable--start-marker-position table)))
          (goto-char marker)
        (goto-char start)
        nil))))

(defun vtable-goto-column (column)
  "Go to COLUMN on the current line."
  (beginning-of-line)
  (if-let* ((match (text-property-search-forward 'vtable-column column t)))
      (goto-char (prop-match-beginning match))
    (end-of-line)))

(defun vtable--display-row-start (table line-number object)
  "Return TABLE's displayed row start at LINE-NUMBER if it stores OBJECT.
LINE-NUMBER indexes the current cache order.  Narrowing to TABLE prevents a
missing row from being mistaken for an identical object in an adjacent table."
  (save-excursion
    ;; Validate that TABLE is fully accessible, then constrain row lookup to
    ;; its exact bounds before counting cache rows.
    (let* ((bounds (vtable--accessible-table-bounds table))
           (table-start (car bounds))
           (table-end (cdr bounds)))
      (save-restriction
        (narrow-to-region table-start table-end)
        (goto-char table-start)
        ;; A direct header occupies one buffer line.  Header-line mode and
        ;; auto-generated columns do not.
        (when (and (slot-value table '-has-column-spec)
                   (not (vtable-use-header-line table)))
          (forward-line 1))
        (let ((remaining (forward-line line-number)))
          (when (and (zerop remaining)
                     (< (point) (point-max))
                     (eq (get-text-property (line-end-position) 'vtable) table)
                     (eq (get-text-property (line-end-position) 'vtable-object)
                         object))
            (line-beginning-position)))))))

(defun vtable-update-object (table object &optional old-object)
  "Update OBJECT's representation in TABLE.
If OLD-OBJECT is non-nil, update the first matching object and display it.
An `eq' match takes precedence; otherwise the first `equal' match is used.
If no matching cached object is found, signal an error without changing TABLE.

TABLE must be at point in the current buffer."
  (unless old-object
    (setq old-object object))
  (let* ((objects (vtable-objects table))
         (object-index (or (seq-position objects old-object #'eq)
                           (seq-position objects old-object #'equal))))
    (unless object-index
      (error "Can't find the old object"))
    (let* ((target-object (nth object-index objects))
           (cache (vtable--current-cache table))
           (cache-lines (and cache (vtable--cache-lines cache)))
           (line-number
            (and cache-lines
                 (seq-position cache-lines target-object
                               (lambda (line object)
                                 (eq (car line) object)))))
           (line (and line-number (nth line-number cache-lines))))
      (unless line
        (error "Can't find cached object in vtable"))
      ;; Normalize `vtable-buffer', which can be either a buffer or its name.
      (let* ((buffer (and (vtable-buffer table)
                          (get-buffer (vtable-buffer table))))
             (display-start
              (and buffer (buffer-live-p buffer)
                   (with-current-buffer buffer
                     (vtable--display-row-start table line-number
                                                target-object)))))
        ;; Validate the exact display row before changing the model/cache or
        ;; invoking user getters.  Cache order plus the row-end EQ check
        ;; distinguishes consecutive rows with the same object identity.
        (unless display-start
          (error "Can't find displayed object in vtable"))
        (let ((display-marker
               (with-current-buffer buffer (copy-marker display-start))))
          (unwind-protect
              (progn
                (if (zerop object-index)
                    (setf (vtable-objects table) (cons object (cdr objects)))
                  (setcar (nthcdr object-index objects) object))
                (with-current-buffer buffer
                  (let ((inhibit-read-only t)
                        (inhibit-modification-hooks t)
                        (old-values (cdr line)))
                    (setcar line object)
                    (setcdr line (vtable--compute-cached-line table object))
                    ;; ... and redisplay the exact cache row selected above.
                    (save-excursion
                      (goto-char display-marker)
                      (let ((keymap (get-text-property (point) 'keymap))
                            (start (point)))
                        (delete-line)
                        (vtable--insert-line table line line-number
                                             (vtable--cache-widths cache)
                                             (vtable--spacer table))
                        (add-text-properties start (point) (list 'keymap keymap
                                                                 'vtable table))))
                    ;; The object's values may have changed between numerical
                    ;; and non-numerical, so recompute.
                    (let ((result
                           (vtable--recompute-numerical
                            table (cdr line) old-values)))
                      (vtable--clear-other-caches
                       table (slot-value table '-current-cache-key))
                      result))))
            (set-marker display-marker nil)))))))
(defun vtable-remove-object (table object)
  "Remove OBJECT from TABLE.
This will also remove the displayed line."
  ;; First remove from the objects.
  (setf (vtable-objects table) (delq object (vtable-objects table)))
  ;; Then adjust the cache and display.
  (with-current-buffer (vtable-buffer table)
    (save-excursion
      (vtable-goto-table table)
      (let ((cache (vtable--current-cache table))
            (inhibit-read-only t)
            (inhibit-modification-hooks t))
        (let* ((cache-lines (vtable--cache-lines cache))
               (removed-lines
                (cl-remove-if-not (lambda (line) (eq (car line) object))
                                  cache-lines))
               (remaining-lines
                (cl-remove-if (lambda (line) (eq (car line) object))
                              cache-lines))
               (columns-to-recompute
                (cl-loop for column in (vtable-columns table)
                         for index from 0
                         when (and (not (vtable-column--numerical column))
                                   (cl-some
                                    (lambda (line)
                                      (not (numberp
                                            (car (nth index (cdr line))))))
                                    removed-lines))
                         collect index)))
          (setcar cache remaining-lines)
          (let ((positions nil)
                (result nil))
            ;; vtable-object can start after a column divider, and equal
            ;; adjacent values can share one text-property run.  Find rows
            ;; by inspecting each line's end before deleting any of them.
            (save-excursion
              (vtable-goto-table table)
              (save-restriction
                (narrow-to-region
                 (point) (save-excursion (vtable-end-of-table)))
                (while (< (point) (point-max))
                  (when (eq (get-text-property (line-end-position)
                                               'vtable-object)
                            object)
                    (push (line-beginning-position) positions))
                  (forward-line 1))))
            ;; Positions are collected top-to-bottom, so PUSH leaves them
            ;; in the order needed to delete from bottom to top.
            (dolist (position positions)
              (goto-char position)
              (setq result (delete-line)))
            ;; Removing a row can leave every value in a column numerical.
            (vtable--recompute-numerical-columns table columns-to-recompute)
            (vtable--clear-other-caches
             table (slot-value table '-current-cache-key))
            result))))))

;; FIXME: The fact that the `location' argument of
;; `vtable-insert-object' can be an integer and is then interpreted as
;; an index precludes the use of integers as objects.  This seems a very
;; unlikely use-case, so let's just accept this limitation.

(defun vtable--insert-object-in-list (table object location before)
  "Add OBJECT to TABLE's backing object list at LOCATION."
  (let ((objects (vtable-objects table)))
    (if (null objects)
        (setf (vtable-objects table) (list object))
      (let ((pos (if location
                     (if (integerp location)
                         (nthcdr location objects)
                       (or (memq location objects)
                           (and before objects)))
                   (if before objects))))
        (if (or before (and pos (integerp location)))
            (let ((old-object (car pos)))
              (setcar pos object)
              (setcdr pos (cons old-object (cdr pos))))
          (if pos
              (setcdr pos (cons object (cdr pos)))
            (nconc objects (list object))))))))

(defun vtable-insert-object (table object &optional location before)
  "Insert OBJECT into TABLE at LOCATION.
LOCATION is an object in TABLE.  OBJECT is inserted after LOCATION,
unless BEFORE is non-nil, in which case it is inserted before LOCATION.

If LOCATION is nil, or does not exist in the table, OBJECT is inserted
at the end of the table, or at the beginning if BEFORE is non-nil.

LOCATION can also be an integer, a (zero-based) index into the table.
OBJECT is inserted at this location.  If the index is out of range,
OBJECT is inserted at the beginning (if the index is less than 0) or
end (if the index is too large) of the table.  BEFORE is ignored in this
case.

This also updates the displayed table, if TABLE has already been inserted."
  ;; Refuse to change TABLE until its complete display is accessible.
  (when (vtable-buffer table)
    (with-current-buffer (vtable-buffer table)
      (vtable--accessible-table-bounds table)))
  ;; Integer LOCATION determines the insertion side; BEFORE applies only
  ;; to object locations and nil LOCATION.
  (when (integerp location)
    (setq before nil))
  ;; When INSERT was nil, update the model only.  vtable-insert will
  ;; create the display cache when the caller chooses an insertion point.
  (if (null (vtable-buffer table))
      (progn
        (vtable--insert-object-in-list table object location before)
        (vtable--recompute-numerical
         table (vtable--compute-cached-line table object)))
    ;; If the vtable is empty, add its first object and regenerate the
    ;; table at the recorded insertion boundary.
    (if (null (vtable-objects table))
      (progn
        (vtable--insert-object-in-list table object location before)
        (vtable--recompute-numerical table (vtable--compute-cached-line table object))
        (with-current-buffer (vtable-buffer table)
          (vtable-goto-table table)
          ;; Insertion changes the model directly.  Re-querying an
          ;; objects-function here would discard the newly inserted row.
          (vtable--clear-cache table)
          (vtable-revert table)))
    ;; First update the backing object list.
    (vtable--insert-object-in-list table object location before)
    ;; Then adjust the cache and display.
    (let* ((cache (vtable--current-cache table))
           (lines (vtable--cache-lines cache))
           (elem (if location  ; This binding mirrors the binding of `pos' above.
                     (if (integerp location)
                         (nth location lines)
                       (or (assq location lines)
                           (and before (car lines))))
                   (if before (car lines))))
           (pos (memq elem lines))
           (line (cons object (vtable--compute-cached-line table object))))
      (with-current-buffer (vtable-buffer table)
        (let ((inhibit-read-only t)
              (inhibit-modification-hooks t))
          (save-excursion
            (vtable-goto-table table)
            (if (or before
                    (and pos (integerp location)))
                ;; Add the new object before:.
                (let ((old-line (car pos)))
                  (setcar pos line)
                  (setcdr pos (cons old-line (cdr pos)))
                  (unless (vtable-goto-object (car elem))
                    (vtable-beginning-of-table)))
              ;; Otherwise, add the object after.
              (if pos
                  ;; Splice the object into the list.
                  (progn
                    (setcdr pos (cons line (cdr pos)))
                    (if (vtable-goto-object location)
                        (forward-line 1)  ; Insert *after*.
                      (vtable-end-of-table)))
                ;; Otherwise, append the object.
                (setcar cache (nconc lines (list line)))
                (vtable-end-of-table)))
            (let* ((start (point))
                   (keymap (get-text-property (point) 'keymap)))
              ;; FIXME: We have to adjust colors in lines below this if we
              ;; have :row-colors.
              (vtable--insert-line table line 0
                                   (vtable--cache-widths cache)
                                   (vtable--spacer table)
                                   (vtable--ellipsis table)
                                   (string-pixel-width
                                    (vtable--ellipsis table) (current-buffer)))
              (add-text-properties start (point) (list 'keymap keymap
                                                       'vtable table)))
            ;; We may have inserted a non-numerical value into a previously
            ;; all-numerical table, so recompute.
            (let ((result (vtable--recompute-numerical table (cdr line))))
              (vtable--clear-other-caches
               table (slot-value table '-current-cache-key))
              result))))))))

(defun vtable-column (table index)
  "Return the name of the INDEXth column in TABLE."
  (vtable-column-name (elt (vtable-columns table) index)))

;;; Generating the table.

(defun vtable--get-value (object index column table)
  "Compute a cell value."
  (cond
   ((vtable-column-getter column)
    (funcall (vtable-column-getter column)
             object table))
   ((vtable-getter table)
    (funcall (vtable-getter table)
             object index table))
   ;; No getter functions; standard getters.
   ((stringp object)
    object)
   (t
    (elt object index))))

(defun vtable--compute-columns (table &optional recompute)
  "Compute column specs for TABLE.
Set the `align', `-aligned' and `-numerical' properties of each column.
If the column contains only numerical data, set `-numerical' to t,
otherwise to nil.  `-aligned' indicates whether the column has an
`align' property set by the user.  If it does, `align' is not touched,
otherwise it is set to `right' for numeric columns and to `left' for
non-numeric columns.

If RECOMPUTE is non-nil, do not set `-aligned'.  This can be used to
recompute the column specs when the table data has changed."
  (let ((numerical (make-vector (length (vtable-columns table)) t))
        (columns (vtable-columns table)))
    ;; First determine whether there are any all-numerical columns.
    (dolist (object (vtable-objects table))
      (seq-do-indexed
       (lambda (_elem index)
         (unless (numberp (vtable--get-value object index (elt columns index)
                                             table))
           (setf (elt numerical index) nil)))
       (vtable-columns table)))
    ;; Check if any columns have an explicit `align' property.
    (unless recompute
      (dolist (column (vtable-columns table))
        (when (vtable-column-align column)
          (setf (vtable-column--aligned column) t))))
    ;; Then fill in defaults.
    (seq-map-indexed
     (lambda (column index)
       ;; This is used when displaying.
       (unless (vtable-column--aligned column)
         (setf (vtable-column-align column)
               (if (elt numerical index)
                   'right
                 'left)))
       ;; This is used for sorting.
       (setf (vtable-column--numerical column)
             (elt numerical index))
       column)
     (vtable-columns table))))

(defun vtable--ellipsis (table)
  (let ((ellipsis (vtable-ellipsis table)))
    (pcase ellipsis
      ((pred (stringp)) ellipsis)
      ('nil "")
      (_ (truncate-string-ellipsis)))))

(defun vtable--spacer (table)
  (vtable--compute-width table (vtable-separator-width table)))

(defun vtable--cache-widths (cache)
  (nth 1 cache))

(defun vtable--cache-lines (cache)
  (car cache))

(defun vtable--insert (table)
  (let* ((table-start (point))
         (spacer (vtable--spacer table))
         (start (point))
         (ellipsis (propertize (vtable--ellipsis table)
                               'face (vtable-face table)))
         (ellipsis-width (string-pixel-width ellipsis (vtable-buffer table)))
         ;; We maintain a cache per screen/window width, so that we render
         ;; correctly if Emacs is open on two different screens (or the
         ;; user resizes the frame).
         (cache-key (vtable--cache-key table))
         (cache (or (gethash cache-key (slot-value table '-cache))
                    (let* ((data (vtable--compute-cache table))
                           (widths (vtable--compute-widths table data)))
                      (setf (gethash cache-key (slot-value table '-cache))
                            (list data widths)))))
         (widths (vtable--cache-widths cache)))
    ;; Don't insert any header or header line if the user hasn't
    ;; specified the columns.
    (when (slot-value table '-has-column-spec)
      (if (vtable-use-header-line table)
          (vtable--set-header-line table widths spacer)
        ;; Insert the header line directly into the buffer, and put a
        ;; keymap to be able to sort the columns there (by clicking on
        ;; them).
        (vtable--insert-header-line table widths spacer)
        (add-text-properties start (point)
                             (list 'keymap vtable-header-line-map
                                   'rear-nonsticky t
                                   'vtable table))
        (setq start (point))))
    (vtable--sort table cache)
    ;; Insert the data.
    (let ((line-number 0))
      (dolist (line (vtable--cache-lines cache))
        (vtable--insert-line table line line-number widths spacer
                             ellipsis ellipsis-width)
        (setq line-number (1+ line-number))))
    (add-text-properties start (point)
                         (list 'rear-nonsticky t
                               'vtable table))
    (setf (vtable--current-cache table) cache
          (slot-value table '-current-cache-key) cache-key)
    (vtable--set-start-marker table table-start (current-buffer))
    (goto-char start)))

(defun vtable-insert (table)
  "Insert TABLE into the current buffer.
The current buffer will be recorded as TABLE's buffer.  If the table is
inserted into a buffer other than its originating buffer, signal an
error.  A table may be reinserted into its own buffer, but insert only
one instance per buffer.  This restriction needs to be enforced by the
caller."
  (if-let* ((table-buffer (vtable-buffer table)))
      (when (not (eq table-buffer (current-buffer)))
        (error "A vtable cannot be inserted into more than one buffer")))
  (setf (vtable-buffer table) (current-buffer))
  (vtable--set-start-marker table (point) (current-buffer))
  (let ((inhibit-read-only t)
        (inhibit-modification-hooks t))
    (vtable--insert table)))

(defun vtable-set-buffer (table buffer)
  "Associate BUFFER with TABLE.
Use this function with care, and ensure your vtable instance renders
itself in the new buffer."
  (let ((buffer-object (and buffer (get-buffer buffer))))
    (setf (vtable-buffer table) buffer)
    (if buffer-object
        (with-current-buffer buffer-object
          (vtable--set-start-marker table (point) buffer-object))
      (vtable--set-start-marker table nil))))

(defun vtable--insert-line (table line line-number widths spacer
                                  &optional ellipsis ellipsis-width)
  (let ((start (point))
        (buffer (vtable-buffer table))
        (columns (vtable-columns table))
        (column-colors
         (and (vtable-column-colors table)
              (if (vtable-row-colors table)
                  (elt (slot-value table '-cached-colors)
                       (mod line-number (length (vtable-row-colors table))))
                (slot-value table '-cached-colors))))
        (divider (vtable-divider table))
        (keymap (slot-value table '-cached-keymap)))
    (seq-do-indexed
     (lambda (elem index)
       (let ((value (nth 0 elem))
             (column (elt columns index))
             (pre-computed (nth 2 elem)))
         ;; See if we have any formatters here.
         (cond
          ((vtable-column-formatter column)
           (setq value (funcall (vtable-column-formatter column) value)
                 pre-computed nil))
          ((vtable-formatter table)
           (setq value (funcall (vtable-formatter table)
                                value index table)
                 pre-computed nil)))
         (let ((displayed
                ;; Allow any displayers to have their say.
                (cond
                 ((vtable-column-displayer column)
                  (funcall (vtable-column-displayer column)
                           value (elt widths index) table))
                 ((vtable-displayer table)
                  (funcall (vtable-displayer table)
                           value index (elt widths index) table))
                 (pre-computed
                  ;; If we don't have a displayer, use the pre-made
                  ;; (cached) string value.
                  (if (> (nth 1 elem) (elt widths index))
                      (truncate-string-pixelwise pre-computed
                                                 (elt widths index)
                                                 buffer
                                                 ellipsis ellipsis-width)
                    pre-computed))
                 (t
                  (truncate-string-pixelwise value
                                             (elt widths index)
                                             buffer
                                             ellipsis ellipsis-width))))
               (start (point))
               ;; Don't insert the separator after the final column.
               (last (= index (- (length line) 2))))
           (if (eq (vtable-column-align column) 'left)
               (progn
                 (insert displayed)
                 (insert (propertize
                          " " 'display
                          (list 'space
                                :width (list
                                        (+ (- (elt widths index)
                                              (string-pixel-width
                                               displayed buffer))
                                           (if last 0 spacer)))))))
             ;; Align to the right.
             (insert (propertize " " 'display
                                 (list 'space
                                       :width (list (- (elt widths index)
                                                       (string-pixel-width
                                                        displayed buffer)))))
                     displayed)
             (unless last
               (insert (propertize " " 'display
                                   (list 'space
                                         :width (list spacer))))))
           (put-text-property start (point) 'vtable-column index)
           (put-text-property start (point) 'keymap keymap)
           (when column-colors
             (add-face-text-property
              start (point)
              (elt column-colors (mod index (length column-colors)))))
           (when (and divider (not last))
             (insert divider)
             (setq start (point))))))
     (cdr line))
    (insert "\n")
    (put-text-property start (point) 'vtable-object (car line))
    (unless column-colors
      (when-let* ((row-colors (slot-value table '-cached-colors)))
        (add-face-text-property
         start (point)
         (elt row-colors (mod line-number (length row-colors))))))))

(defun vtable--cache-key-snapshot (object)
  "Return a detached, complete snapshot of OBJECT for a cache key."
  (let ((print-level nil)
        (print-length nil)
        (print-circle t))
    (prin1-to-string object)))

(defun vtable--buffer-pixel-context (buffer)
  "Return a detached snapshot of BUFFER's display-property context."
  (let ((buffer (cond ((bufferp buffer) buffer)
                      ((stringp buffer) (get-buffer buffer)))))
    (when (and buffer (buffer-live-p buffer))
      (with-current-buffer buffer
        (vtable--cache-key-snapshot
         (mapcar (lambda (variable)
                   (list variable
                         (local-variable-p variable buffer)
                         (if (boundp variable)
                             (symbol-value variable)
                           :unbound)))
                 '(face-remapping-alist
                   char-property-alias-alist
                   default-text-properties)))))))

(defun vtable--cache-key (&optional table)
  "Return the selected frame's layout key for TABLE.
The cdr retains the character-column width for compatibility with callers
that derive alternate width keys from it."
  (let* ((window (selected-window))
         (frame (window-frame window))
         (face (if table (vtable-face table) 'default))
         (character-pixel-width
          (if table
              (vtable--char-width table)
            (string-pixel-width "x")))
         (layout (list (frame-terminal frame)
                       frame
                       (window-width window t)
                       (display-graphic-p frame)
                       vtable-tty-char-pixel-width
                       character-pixel-width
                       (vtable--cache-key-snapshot face)
                       (vtable--cache-key-snapshot
                        (frame-parameter frame 'font))
                       (vtable--cache-key-snapshot
                        (frame-parameter frame 'fontset))
                       (frame-parameter frame 'scale-factor)
                       (and table
                            (vtable--buffer-pixel-context
                             (vtable-buffer table))))))
    (cons (list 'vtable-cache-key layout)
          (window-width window))))

(defun vtable--clear-cache (table)
  (setf (gethash (vtable--cache-key table) (slot-value table '-cache)) nil))

(defun vtable--clear-other-caches (table keep-key)
  "Discard cached layouts of TABLE except KEEP-KEY.
If KEEP-KEY is nil, discard every cached layout.  Invalidation is keyed:
two keys can refer to the same cache object but still denote separate widths."
  (let ((cache-table (slot-value table '-cache))
        keys)
    (maphash (lambda (key _cache)
               (unless (and keep-key (equal key keep-key))
                 (push key keys)))
             cache-table)
    (dolist (key keys)
      (remhash key cache-table))))

(defun vtable--sort (table cache)
  (pcase-dolist (`(,index . ,direction) (vtable-sort-by table))
    (let ((numerical (vtable-column--numerical
                      (elt (vtable-columns table) index)))
          (numcomp (if (eq direction 'descend)
                       #'> #'<))
          (stringcomp (if (eq direction 'descend)
                          #'string> #'string<)))
      (setcar cache
              (sort (car cache)
                    (lambda (e1 e2)
                      (let ((c1 (elt e1 (1+ index)))
                            (c2 (elt e2 (1+ index))))
                        (if numerical
                            (funcall numcomp (car c1) (car c2))
                          (funcall
                           stringcomp
                           (if (stringp (car c1))
                               (car c1)
                             (format "%s" (car c1)))
                           (if (stringp (car c2))
                               (car c2)
                             (format "%s" (car c2))))))))))))

(defun vtable--indicator (table index)
  (let ((order (car (last (vtable-sort-by table)))))
    (if (eq index (car order))
        ;; We're sorting by this column last, so return an indicator.
        (catch 'found
          (dolist (candidate (nth (if (eq (cdr order) 'ascend)
                                      1
                                    0)
                                  '((?▼ ?v)
                                    (?▲ ?^))))
            (when (char-displayable-p candidate)
              (throw 'found (string candidate)))))
      "")))

(defun vtable--insert-header-line (table widths spacer)
  ;; Insert the header directly into the buffer.
  (let ((start (point))
        (buffer (vtable-buffer table))
        (divider (vtable-divider table))
        (cmap (define-keymap
                "<header-line> <drag-mouse-1>" #'vtable--drag-resize-column
                "<header-line> <down-mouse-1>" #'ignore))
        (dmap (define-keymap
                "<header-line> <drag-mouse-1>"
                (lambda (e)
                  (interactive "e")
                  (vtable--drag-resize-column e t))
                "<header-line> <down-mouse-1>" #'ignore)))
    (seq-do-indexed
     (lambda (column index)
       (let* ((name (propertize
                     (vtable-column-name column)
                     'face (list 'header-line (vtable-face table))
                     'mouse-face 'header-line-highlight
                     'keymap cmap))
              (start (point))
              (indicator (vtable--indicator table index))
              (indicator-width (string-pixel-width indicator buffer))
              (last (= index (1- (length (vtable-columns table)))))
              (displayed (truncate-string-pixelwise
                          name
                          (- (elt widths index) indicator-width)
                          buffer)))
         (let* ((indicator-lead-width
                 (if (display-graphic-p)
                     ;; On a graphical frame, we want the indicator to
                     ;; not be quite flush right...
                     (/ (vtable--char-width table) 2.0)
                   0))
                (indicator-pad-width
                 (if (display-graphic-p)
                     ;; ...and adjust its padding.
                     (- (vtable--char-width table)
                        indicator-lead-width)
                   0))
                (fill-width
                 (+ (- (elt widths index)
                       (string-pixel-width displayed buffer)
                       indicator-width
                       indicator-lead-width)
                    (if last 0 spacer))))
           (if (or (not last)
                   (zerop indicator-width)
                   (< (seq-reduce #'+ widths 0) (window-width nil t)))
               ;; Normal case.
               (insert
                displayed
                (propertize " " 'display
                            (list 'space :width (list fill-width)))
                indicator
                (propertize " " 'display
                            (list 'space :width (list indicator-pad-width))))
             ;; This is the final column, and we have a sorting
             ;; indicator, and the table is too wide for the window.
             (let* ((pre-indicator (string-pixel-width
                                    (buffer-substring (point-min) (point))
                                    buffer))
                    (pre-fill
                     (- (window-width nil t)
                        pre-indicator
                        (string-pixel-width displayed))))
               (insert
                displayed
                (propertize " " 'display
                            (list 'space :width (list pre-fill)))
                indicator
                (propertize " " 'display
                            (list 'space :width
                                  (list (- fill-width pre-fill))))))))
         (when (and divider (not last))
           (insert (propertize divider 'keymap dmap)))
         (put-text-property start (point) 'vtable-column index)))
     (vtable-columns table))
    (insert "\n")
    (add-face-text-property start (point) 'header-line)))

(defun vtable--drag-resize-column (e &optional next)
  "Resize the column by dragging.
If NEXT, do the next column."
  (interactive "e")
  (let* ((pos-start (event-start e))
	 (obj (posn-object pos-start)))
    (with-current-buffer (window-buffer (posn-window pos-start))
      (let ((column
             ;; In the header line we have a text property on the
             ;; divider.
             (or (get-text-property (if obj (cdr obj)
                                      (posn-point pos-start))
			            'vtable-column
			            (car obj))
                 ;; For reasons of efficiency, we don't have that in
                 ;; the buffer itself, so find the column.
                 (save-excursion
                   (goto-char (posn-point pos-start))
                   (1+
                    (get-text-property
                     (prop-match-beginning
                      (text-property-search-backward 'vtable-column))
                     'vtable-column)))))
            (start-x (car (posn-x-y pos-start)))
            (end-x (car (posn-x-y (event-end e)))))
        (when (or (> column 0) next)
          (vtable--alter-column-width (vtable-current-table)
                                      (if next
                                          column
                                        (1- column))
                                      (- end-x start-x)))))))

(defun vtable--recompute-numerical (table line &optional old-values)
  "Recompute numericalness of columns if necessary.
OLD-VALUES, when non-nil, are the cached cells before a row update."
  (let ((columns (vtable-columns table))
        (indices nil))
    (seq-do-indexed
     (lambda (elem index)
       (when (or (and old-values
                      (not (eq (numberp (car elem))
                               (numberp (car (nth index old-values))))))
                 (and (vtable-column--numerical (elt columns index))
                      (not (numberp (car elem)))))
         (push index indices)))
     line)
    (vtable--recompute-numerical-columns table (nreverse indices) line)))

(defun vtable--recompute-numerical-columns (table indices &optional new-line)
  "Recompute numerical metadata for INDICES from the current cache.
When non-nil, include NEW-LINE even if it is not in the cache yet.
This avoids calling user getters again after a row has been rendered."
  (let* ((cache (vtable--current-cache table))
         (cache-lines (and cache (vtable--cache-lines cache)))
         (lines (append cache-lines
                        (and new-line (list (cons nil new-line)))))
         (columns (vtable-columns table)))
    (dolist (index indices)
      (let* ((column (elt columns index))
             (numerical
              (seq-every-p (lambda (line)
                             (numberp (car (nth index (cdr line)))))
                           lines)))
        (setf (vtable-column--numerical column) numerical)
        (unless (vtable-column--aligned column)
          (setf (vtable-column-align column)
                (if numerical 'right 'left)))))
    (when indices
      (vtable-columns table))))

(defun vtable--set-header-line (table widths spacer)
  (setq header-line-format
        (string-replace
         "%" "%%"
         (with-temp-buffer
           (insert " ")
           (vtable--insert-header-line table widths spacer)
           ;; Align the header with the (possibly) fringed buffer text.
           (put-text-property
            (point-min) (1+ (point-min))
            'display '(space :align-to 0))
           (buffer-substring (point-min) (1- (point-max))))))
  (vtable-header-mode 1))


(defun vtable--char-width (table)
  (string-pixel-width (propertize "x" 'face (vtable-face table))
                      (vtable-buffer table)))

(defconst vtable-tty-char-pixel-width 10
  "This is a conversion factor from pixels to characters for tty terminals.
This represents the average number of pixels of a character displayed on
a tty terminal.
This is applied if a column's width is specified in pixels and the
vtable is being rendered on a tty terminal.")

(defun vtable--compute-width (table spec)
  (cond
   ((numberp spec)
    (* spec (vtable--char-width table)))
   ((string-match "\\([0-9.]+\\)ex" spec)
    (* (string-to-number (match-string 1 spec)) (vtable--char-width table)))
   ((string-match "\\([0-9.]+\\)px" spec)
    (/ (string-to-number (match-string 1 spec))
       ;; Adjust pixels down by a factor for tty terminals.
       (if (display-graphic-p) 1 vtable-tty-char-pixel-width)))
   ((string-match "\\([0-9.]+\\)%" spec)
    (/ (* (string-to-number (match-string 1 spec)) (window-width nil t))
       100))
   (t
    (error "Invalid spec: %s" spec))))

(defun vtable--compute-widths (table cache)
  "Compute the display widths for TABLE.
CACHE is TABLE's cache data as returned by `vtable--compute-cache'."
  (let* ((n-0cols 0) ; Count the number of zero-width columns.
         (widths (seq-map-indexed
                  (lambda (column index)
                    (let ((width
                           (or
                            ;; Explicit widths.
                            (and (vtable-column-width column)
                                 (vtable--compute-width table (vtable-column-width column)))
                            ;; If the vtable is empty and no explicit width is given,
                            ;; set its width to 0 and deal with it below.
                            (when (null cache)
                              (setq n-0cols (1+ n-0cols))
                              0)
                            ;; Otherwise, compute based on the displayed widths of the
                            ;; data.
                            (seq-max (seq-map (lambda (elem)
                                                (nth 1 (elt (cdr elem) index)))
                                              cache)))))
                      ;; Let min-width/max-width specs have their say.
                      (when-let* ((min-width (and (vtable-column-min-width column)
                                                 (vtable--compute-width
                                                  table (vtable-column-min-width column)))))
                        (setq width (max width min-width)))
                      (when-let* ((max-width (and (vtable-column-max-width column)
                                                 (vtable--compute-width
                                                  table (vtable-column-max-width column)))))
                        (setq width (min width max-width)))
                      width))
                  (vtable-columns table))))
    ;; If there are any zero-width columns, divide the remaining window
    ;; width evenly over them.
    (when (> n-0cols 0)
      (let* ((combined-width (apply #'+ widths))
             (default-width (/ (- (window-width nil t) combined-width) n-0cols)))
        (setq widths (mapcar (lambda (width)
                               (if (zerop width)
                                   default-width
                                 width))
                             widths))))
    (seq-into widths 'vector)))

(defun vtable--compute-cache (table)
  (seq-map
   (lambda (object)
     (cons object (vtable--compute-cached-line table object)))
   (vtable-objects table)))

(defun vtable--compute-cached-line (table object)
  (seq-map-indexed
   (lambda (column index)
     (let* ((value (vtable--get-value object index column table))
            (string (if (stringp value)
                        (copy-sequence value)
                      (format "%s" value))))
       (add-face-text-property 0 (length string)
                               (vtable-face table)
                               t string)
       ;; We stash the computed width and string here -- if there are
       ;; no formatters/displayers, we'll be using the string, and
       ;; then won't have to recreate it.
       (list value (string-pixel-width string (vtable-buffer table)) string)))
   (vtable-columns table)))

(defun vtable--make-keymap (table)
  (let ((map (if (or (vtable-actions table)
                     (vtable-keymap table))
                 (copy-keymap vtable-map)
               vtable-map)))
    (when-let* ((actions (vtable-actions table)))
      (while actions
        (funcall (lambda (key binding)
                   (keymap-set map key
                               (lambda (object)
                                 (interactive (list (vtable-current-object)))
                                 (funcall binding object))))
                 (car actions) (cadr actions))
        (setq actions (cddr actions))))
    (if (vtable-keymap table)
        (progn
          (setf (vtable-keymap table)
                (copy-keymap (vtable-keymap table)))
          ;; Respect any previously set parent keymaps.
          (set-keymap-parent (vtable-keymap table)
                             (if (keymap-parent (vtable-keymap table))
                                 (append (ensure-list
                                          (vtable-keymap table))
                                         (list map))
                               map))
          (vtable-keymap table))
      map)))

(defun vtable--table-bounds (table)
  "Return TABLE's full-buffer bounds as (START . END)."
  (save-excursion
    (save-restriction
      (widen)
      (unless (vtable-goto-table table)
        (user-error "No table insertion boundary"))
      (let* ((start (vtable--beginning-of-table table))
             (end (vtable--end-of-table table)))
        (cons start end)))))

(defun vtable--accessible-table-bounds (table)
  "Return TABLE's bounds if it lies wholly inside the accessible region."
  (let* ((accessible-min (point-min))
         (accessible-max (point-max))
         (bounds (vtable--table-bounds table))
         (start (car bounds))
         (end (cdr bounds)))
    (unless (and (<= accessible-min start) (<= end accessible-max))
      (user-error "Table is not fully accessible"))
    bounds))

(defun vtable-revert (&optional table)
  "Regenerate TABLE, defaulting to the table under point."
  (setq table (or table (vtable-current-table)))
  (unless table
    (user-error "No table under point"))
  (unless (vtable-buffer table)
    (user-error "Table not inserted"))
  (with-current-buffer (vtable-buffer table)
    (let ((object (vtable-current-object))
          (column (vtable-current-column))
          (inhibit-read-only t)
          (inhibit-modification-hooks t))
      (let* ((bounds (vtable--accessible-table-bounds table))
             (start (car bounds))
             (end (cdr bounds)))
        (let* ((key (vtable--cache-key table))
               (keep-key (and (gethash key (slot-value table '-cache)) key)))
          (vtable--clear-other-caches table keep-key))
        (delete-region start end)
        (goto-char start)
        (vtable--insert table))
      (when object
        (vtable-goto-object object))
      (when column
        (vtable-goto-column column)))))
;;; Commands.

(defvar-keymap vtable-header-mode-map
  "<header-line> <mouse-1>" 'vtable-header-line-sort
  "<header-line> <mouse-2>" 'vtable-header-line-sort)

(define-minor-mode vtable-header-mode
  "Minor mode for buffers with vtables with headers."
  :keymap vtable-header-mode-map)

(defun vtable-narrow-current-column (&optional n)
  "Narrow the current column by N characters.
If N isn't given, N defaults to 1.

Interactively, N is the prefix argument."
  (interactive "p")
  (let* ((table (vtable-current-table))
         (column (vtable-current-column)))
    (unless column
      (user-error "No column under point"))
    (vtable--alter-column-width table column
                                (- (* (vtable--char-width table) (or n 1))))))

(defun vtable--alter-column-width (table column delta)
  (let* ((widths (vtable--cache-widths (vtable--current-cache table)))
         (new-width (max (* (vtable--char-width table) 2)
                         (+ (aref widths column) delta))))
    (setf (aref widths column) new-width)
    ;; Store the width so it'll be respected on a revert.
    (setf (vtable-column-width (elt (vtable-columns table) column))
          (if (display-graphic-p)
              (format "%dpx" new-width)
            new-width))
    (vtable-revert table)))

(defun vtable-widen-current-column (&optional n)
  "Widen the current column by N characters.
If N isn't given, N defaults to 1.

Interactively, N is the prefix argument."
  (interactive "p")
  (vtable-narrow-current-column (- n)))

(defun vtable-previous-column ()
  "Go to the previous column."
  (interactive)
  (vtable-goto-column
   (max 0 (1- (or (vtable-current-column)
                  (length (vtable--cache-widths
                           (vtable--current-cache (vtable-current-table)))))))))

(defun vtable-next-column ()
  "Go to the next column."
  (interactive)
  (when (vtable-current-column)
    (vtable-goto-column
     (min (1- (length (vtable--cache-widths
                       (vtable--current-cache (vtable-current-table)))))
          (1+ (vtable-current-column))))))

(defun vtable-revert-command (&optional table)
  "Re-query data and regenerate TABLE.
If TABLE is nil, use the table under point."
  (interactive)
  (setq table (or table (vtable-current-table)))
  (unless table
    (user-error "No table found"))
  (unless (vtable-buffer table)
    (user-error "Table not inserted"))
  (with-current-buffer (vtable-buffer table)
    (vtable--accessible-table-bounds table))
  (when (vtable-objects-function table)
    (setf (vtable-objects table) (funcall (vtable-objects-function table))))
  (vtable--clear-cache table)
  (vtable-revert table))

(defun vtable-sort-by-current-column ()
  "Sort the table under point by the column under point."
  (interactive)
  (unless (vtable-current-column)
    (user-error "No current column"))
  (let* ((table (vtable-current-table))
         (last (car (last (vtable-sort-by table))))
         (index (vtable-current-column)))
    ;; First prune any previous appearance of this column.
    (setf (vtable-sort-by table)
          (delq (assq index (vtable-sort-by table))
                (vtable-sort-by table)))
    ;; Then insert this as the last sort key.
    (setf (vtable-sort-by table)
          (append (vtable-sort-by table)
                  (list (cons index
                              (if (eq (car last) index)
                                  (if (eq (cdr last) 'ascend)
                                      'descend
                                    'ascend)
                                'ascend)))))
    (vtable-revert table)))

(defun vtable-header-line-sort (e)
  "Sort a vtable from the header line."
  (interactive "e")
  (let* ((pos (event-start e))
	 (obj (posn-object pos)))
    (with-current-buffer (window-buffer (posn-window pos))
      (goto-char (point-min))
      (vtable-goto-column
       (get-text-property (if obj (cdr obj) (posn-point pos))
			  'vtable-column
			  (car obj)))
      (vtable-sort-by-current-column))))

(provide 'vtable)

;;; vtable.el ends here
