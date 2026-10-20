;;; -*- lexical-binding: t; -*-
;; GNU fns.c compares live overlay bounds/buffer, then its ordinary plist.
(defmacro qa128-overlays (&rest body)
  `(with-temp-buffer
     (insert "abcdef")
     (let ((a (make-overlay 2 4)) (b (make-overlay 2 4))) ,@body)))
(qa (qa128-overlays (eq a b)) nil)
(qa (qa128-overlays (eql a b)) nil)
(qa (qa128-overlays (equal a b)) t)
(qa (qa128-overlays (equal-including-properties a b)) t)
(qa (qa128-overlays (move-overlay b 3 4) (equal a b)) nil)
(qa (qa128-overlays (move-overlay b 2 5) (equal a b)) nil)
(qa (qa128-overlays
      (let ((b (make-overlay 2 4 nil t t))) (equal a b))) t)
(qa (qa128-overlays (overlay-put a 'tag 1) (equal a b)) nil)
(qa (qa128-overlays (overlay-put a 'tag nil) (equal a b)) nil)
(qa (qa128-overlays
      (overlay-put a 'tag (list 1 2)) (overlay-put b 'tag (list 1 2)) (equal a b)) t)
(qa (qa128-overlays
      (overlay-put a 'tag 1) (overlay-put b 'tag 2) (equal a b)) nil)
(qa (qa128-overlays
      (overlay-put a 'x 1) (overlay-put a 'y 2)
      (overlay-put b 'y 2) (overlay-put b 'x 1) (equal a b)) nil)
(qa (qa128-overlays (equal a (copy-overlay a))) t)
(qa (qa128-overlays (equal (list a) (list b))) t)
(qa (qa128-overlays (equal (vector a) (vector b))) t)
(qa (qa128-overlays (eq (car (member b (list a))) a)) t)
(qa (qa128-overlays (eq (caar (list (assoc b (list (cons a 'value))))) a)) t)
(qa (qa128-overlays
      (let ((table (make-hash-table :test 'equal)))
        (puthash a 'found table) (gethash b table))) 'found)
(qa (qa128-overlays
      (let ((table (make-hash-table :test 'equal)))
        (puthash a 1 table) (puthash b 2 table)
        (list (hash-table-count table) (gethash a table)))) '(1 2))
(qa (qa128-overlays
      (let ((table (make-hash-table :test 'equal)))
        (puthash (list a) 1 table) (gethash (list b) table))) 1)
(qa (qa128-overlays
      (let ((table (make-hash-table :test 'equal)))
        (puthash a 1 table) (remhash b table) (hash-table-count table))) 0)
(qa (qa128-overlays
      (let ((table (make-hash-table :test 'eq)))
        (puthash a 1 table) (puthash b 2 table) (hash-table-count table))) 2)
(qa (qa128-overlays (= (sxhash-equal a) (sxhash-equal b))) t)
(qa (qa128-overlays
      (move-overlay b 4 6) (delete-overlay a) (delete-overlay b) (equal a b)) t)
(qa (qa128-overlays
      (delete-overlay a) (equal a b)) nil)
(qa (qa128-overlays
      (delete-overlay a) (delete-overlay b)
      (overlay-put a 'x 1) (overlay-put b 'x 2) (equal a b)) nil)
(qa (qa128-overlays
      (move-overlay b 4 6) (delete-overlay a) (delete-overlay b)
      (list (sxhash-equal a) (sxhash-equal b))) '(20 20))
(qa (let ((a (make-overlay 1 1)))
      (unwind-protect
          (with-temp-buffer (equal a (make-overlay 1 1)))
        (delete-overlay a))) nil)
(qa (let ((a (make-overlay 1 1)))
      (with-temp-buffer
        (let ((b (make-overlay 1 1)))
          (delete-overlay a) (delete-overlay b) (equal a b)))) t)
(qa (qa128-overlays (equal a [overlay 0 0])) nil)
(qa (equal [overlay 0 0] [overlay 0 0]) t)
(qa (qa128-overlays
      (overlay-put a 'x (propertize "x" 'tag 1))
      (overlay-put b 'x (propertize "x" 'tag 2)) (equal a b)) t)
(qa (qa128-overlays
      (overlay-put a 'x (propertize "x" 'tag 1))
      (overlay-put b 'x (propertize "x" 'tag 2)) (equal-including-properties a b)) nil)
(qa (qa128-overlays
      (overlay-put a 'self a) (overlay-put b 'self b) (equal a b)) t)
(qa (qa128-overlays
      (overlay-put a 'self a) (overlay-put b 'self b) (equal-including-properties a b)) t)
(qa (qa128-overlays
      (overlay-put a 'self a) (overlay-put b 'self b)
      (overlay-put a 'other 1) (overlay-put b 'other 2) (equal a b)) nil)
(qa (qa128-overlays
      (overlay-put a 'self a) (overlay-put b 'self b)
      (let ((table (make-hash-table :test 'equal)))
        (puthash a 'found table) (gethash b table))) 'found)
(defun qa128-deep (n)
  (let ((value 0)) (dotimes (_ n value) (setq value (vector value)))))
(qa (qa128-overlays
      (overlay-put a 'deep (qa128-deep 190))
      (overlay-put b 'deep (qa128-deep 190)) (equal a b)) t)
(qa-err-data (qa128-overlays
              (overlay-put a 'deep (qa128-deep 210))
              (overlay-put b 'deep (qa128-deep 210)) (equal a b))
             'error '("Stack overflow in equal"))
(qa (qa128-overlays
      (move-overlay b 3 4)
      (overlay-put a 'deep (qa128-deep 210))
      (overlay-put b 'deep (qa128-deep 210)) (equal a b)) nil)
(qa (qa128-overlays
      (let ((b (make-overlay 2 4 nil t nil)))
        (goto-char 2) (insert "x") (equal a b))) nil)
(princ (qa--done))
