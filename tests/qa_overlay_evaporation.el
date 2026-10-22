;;; qa_overlay_evaporation.el --- Effective evaporation at deletion -*- lexical-binding: t; -*-
;; Category defaults, hooks and shared-text views all participate.

(defun qa310-delete (category-value direct begin end from to &optional mutation hook operation)
  (with-temp-buffer
    (insert "abcdef")
    (let ((category (make-symbol "qa310-category")) (o (make-overlay begin end)))
      (put category 'evaporate category-value)
      (overlay-put o 'category category)
      (unless (eq direct 'absent) (overlay-put o 'evaporate direct))
      (let ((update (lambda ()
                      (when mutation
                        (put category 'evaporate (eq mutation 'turn-on))))))
        (unless hook (funcall update))
        (let ((before-change-functions
               (and hook (list (lambda (&rest _) (funcall update))))))
          (cond ((eq operation 'extract) (delete-and-extract-region from to))
                ((eq operation 'char) (goto-char to) (delete-char (- from to)))
                ((eq operation 'erase) (erase-buffer))
                (t (delete-region from to)))))
      (list (overlay-start o) (overlay-end o) (and (overlay-buffer o) t)
            (overlay-get o 'evaporate)))))

(defun qa310-shared (edit-indirect &optional sibling-only)
  (with-temp-buffer
    (insert "abcdef")
    (let* ((base (current-buffer))
           (view (make-indirect-buffer base " *qa310-view*"))
           (category (make-symbol "qa310-shared"))
           (a (and (not sibling-only) (make-overlay 1 3 base)))
           (b (make-overlay 1 3 view)))
      (unwind-protect
          (progn
            (put category 'evaporate t)
            (when a (overlay-put a 'category category))
            (overlay-put b 'category category)
            (with-current-buffer (if edit-indirect view base) (delete-region 1 3))
            (list (and a (overlay-buffer a) t) (and (overlay-buffer b) t)
                  (and a (overlay-get a 'evaporate)) (overlay-get b 'evaporate)))
        (kill-buffer view)))))

(defun qa310-replace (replacement)
  (with-temp-buffer
    (insert "abcdef")
    (let ((category (make-symbol "qa310-replace")) (o (make-overlay 1 3)))
      (put category 'evaporate t) (overlay-put o 'category category)
      (replace-region-contents 1 3 (lambda () replacement))
      (list (overlay-start o) (overlay-end o) (and (overlay-buffer o) t)
            (overlay-get o 'evaporate) (buffer-string)))))

;; inherited evaporate
(qa (qa310-delete t 'absent 1 3 1 3)
    '(nil nil nil t))

;; direct evaporate
(qa (qa310-delete nil t 1 3 1 3)
    '(nil nil nil t))

;; no evaporate
(qa (qa310-delete nil 'absent 1 3 1 3)
    '(1 1 t nil))

;; explicit nil shadows category
(qa (qa310-delete t nil 1 3 1 3)
    '(1 1 t nil))

;; category changes on
(qa (qa310-delete nil 'absent 1 3 1 3 'turn-on)
    '(nil nil nil t))

;; category changes off
(qa (qa310-delete t 'absent 1 3 1 3 'turn-off)
    '(1 1 t nil))

;; before-change enables evaporation
(qa (qa310-delete nil 'absent 1 3 1 3 'turn-on t)
    '(nil nil nil t))

;; before-change disables evaporation
(qa (qa310-delete t 'absent 1 3 1 3 'turn-off t)
    '(1 1 t nil))

;; partial deletion keeps overlay
(qa (qa310-delete t 'absent 1 4 1 2)
    '(1 3 t t))

;; empty overlay after deletion range
(qa (qa310-delete t 'absent 5 5 1 2)
    '(4 4 t t))

;; empty overlay before deletion range
(qa (qa310-delete t 'absent 1 1 3 4)
    '(1 1 t t))

;; empty overlay at deletion start
(qa (qa310-delete t 'absent 2 2 2 3)
    '(nil nil nil t))

;; empty overlay at old deletion end
(qa (qa310-delete t 'absent 3 3 2 3)
    '(nil nil nil t))

;; empty deletion does not evaporate
(qa (qa310-delete t 'absent 2 2 2 2)
    '(2 2 t t))

;; non-nil zero value
(qa (qa310-delete 0 'absent 1 3 1 3)
    '(nil nil nil 0))

;; extraction path
(qa (qa310-delete t 'absent 1 3 1 3 nil nil 'extract)
    '(nil nil nil t))

;; backward character deletion
(qa (qa310-delete t 'absent 1 3 1 3 nil nil 'char)
    '(nil nil nil t))

;; erase path
(qa (qa310-delete t 'absent 2 5 1 7 nil nil 'erase)
    '(nil nil nil t))

;; base edit updates indirect overlays
(qa (qa310-shared nil)
    '(nil nil t t))

;; indirect edit updates base overlays
(qa (qa310-shared t)
    '(nil nil t t))

;; only sibling has overlays
(qa (qa310-shared nil t)
    '(nil nil nil t))

;; atomic nonempty replacement
(qa (qa310-replace "XY")
    '(1 3 t t "XYcdef"))

;; empty replacement evaporates
(qa (qa310-replace "")
    '(nil nil nil t "cdef"))

;; changed defaults do not leave stale cache
(qa (with-temp-buffer (insert "abcdef") (let ((category (make-symbol "qa310-cache")) (o (make-overlay 2 5))) (put category 'evaporate t) (overlay-put o 'category category) (delete-region 1 2) (put category 'evaporate nil) (move-overlay o 4 4) (delete-region 1 2) (list (overlay-start o) (overlay-end o) (and (overlay-buffer o) t) (overlay-get o 'evaporate))))
    '(3 3 t nil))

(princ (qa--done))
