;;; -*- lexical-binding: nil; -*-
;; Issue #193: equal marker keys use buffer + position, including nested keys.
;; Expected values checked with GNU Emacs 31.1.

(qa (with-temp-buffer
      (let ((h (make-hash-table :test 'equal)))
        (puthash (point-marker) 42 h)
        (gethash (point-marker) h)))
    42)

;; Fresh equal markers replace the value without replacing the original key.
;; Insertion type is not part of marker equality.
(with-temp-buffer
  (insert "aé日z")
  (goto-char 3)
  (let* ((h (make-hash-table :test 'equal))
         (a (point-marker))
         (b (copy-marker a t)))
    (qa (equal a b) t)
    (qa (= (sxhash-equal a) (sxhash-equal b)) t)
    (qa (puthash a 'first h) 'first)
    (qa (gethash b h 'missing) 'first)
    (qa (puthash b 'second h) 'second)
    (qa (hash-table-count h) 1)
    (qa (let (key) (maphash (lambda (k _v) (setq key k)) h) (eq key a)) t)
    (qa (gethash a h) 'second)
    (qa (gethash (copy-marker 2) h 'missing) 'missing)
    (let ((copy (copy-hash-table h)))
      (qa (gethash b copy) 'second)
      (qa (remhash b copy) nil)
      (qa (hash-table-count copy) 0)
      (qa (hash-table-count h) 1))
    (qa (remhash b h) nil)
    (qa (gethash a h 'missing) 'missing)
    (qa (hash-table-count h) 0)))

;; Same position in different buffers is a different key.
(let ((a (generate-new-buffer " *qa-marker-a*"))
      (b (generate-new-buffer " *qa-marker-b*")))
  (unwind-protect
      (let ((h (make-hash-table :test 'equal)))
        (with-current-buffer a (puthash (point-marker) 'a h))
        (with-current-buffer b
          (qa (gethash (point-marker) h 'missing) 'missing)
          (puthash (point-marker) 'b h))
        (qa (hash-table-count h) 2)
        (with-current-buffer a (qa (gethash (point-marker) h) 'a))
        (with-current-buffer b (qa (gethash (point-marker) h) 'b)))
    (kill-buffer a)
    (kill-buffer b)))

;; Detached markers, including a marker detached before insertion.
(let ((h (make-hash-table :test 'equal))
      (a (make-marker))
      (b (with-temp-buffer (insert "abc") (point-marker))))
  (qa (equal a b) t)
  (puthash a 7 h)
  (qa (gethash b h 'missing) 7)
  (puthash (make-marker) 8 h)
  (qa (hash-table-count h) 1)
  (qa (gethash b h) 8)
  (qa (gethash nil h 'missing) 'missing)
  (qa (gethash 0 h 'missing) 'missing)
  (remhash b h)
  (qa (hash-table-count h) 0))

;; Explicitly detached markers retain no meaningful position or insertion type.
(with-temp-buffer
  (insert "abc")
  (let ((a (copy-marker 2 t)) (b (copy-marker 4))
        (h (make-hash-table :test 'equal)))
    (set-marker a nil)
    (set-marker b nil)
    (qa (equal a b) t)
    (qa (equal a (make-marker)) t)
    (qa (= (sxhash-equal a) (sxhash-equal b)) t)
    (puthash (vector a) 'detached h)
    (qa (gethash (vector b) h 'missing) 'detached)
    (puthash (vector (make-marker)) 'updated h)
    (qa (hash-table-count h) 1)
    (remhash (vector b) h)
    (qa (hash-table-count h) 0)))

;; Recursive normalization must use marker contents in every container.
(dolist (wrap (list #'list #'vector
                   (lambda (m) (record 'qa-marker m))
                   (lambda (m) (list (vector m) (cons 'tag m)))))
  (with-temp-buffer
    (let* ((h (make-hash-table :test 'equal))
           (a (funcall wrap (point-marker)))
           (b (funcall wrap (copy-marker (point) t))))
      (qa (equal a b) t)
      (puthash a 'found h)
      (qa (gethash b h 'missing) 'found)
      (puthash b 'updated h)
      (qa (hash-table-count h) 1)
      (qa (gethash a h) 'updated)
      (remhash b h)
      (qa (hash-table-count h) 0))))

;; Cycles use the bounded-hash fallback and still collision-check equality.
(with-temp-buffer
  (let* ((h (make-hash-table :test 'equal))
         (a (list (point-marker)))
         (b (list (point-marker))))
    (setcdr a a)
    (setcdr b b)
    (qa (equal a b) t)
    (puthash a 'cycle h)
    (qa (gethash b h 'missing) 'cycle)
    (puthash b 'updated h)
    (qa (hash-table-count h) 1)
    (remhash b h)
    (qa (hash-table-count h) 0)))

;; eq/eql tables keep distinct marker objects distinct, even if equal.
(dolist (test '(eq eql))
  (with-temp-buffer
    (let ((h (make-hash-table :test test))
          (a (point-marker))
          (b (point-marker)))
      (qa (equal a b) t)
      (puthash a 'a h)
      (qa (gethash a h) 'a)
      (qa (gethash b h 'missing) 'missing)
      (puthash b 'b h)
      (qa (hash-table-count h) 2)
      (remhash b h)
      (qa (gethash a h) 'a)
      (qa (hash-table-count h) 1))))

(princ (qa--done))
