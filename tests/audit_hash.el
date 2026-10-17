;;; audit_hash.el --- hash table differential probes -*- lexical-binding: t; -*-
;; In-memory only; run with -Q --batch -l under GNU 31.1 and remacs.
(defun audit-hash-run (id form)
 (unless (or (and (getenv "REMACS_AUDIT_ONLY")
                  (not (equal id (getenv "REMACS_AUDIT_ONLY"))))
             (member id (split-string (or (getenv "REMACS_AUDIT_SKIP") "") "," t)))
  (let ((print-circle t) (print-length 50) (print-level 20)
        (print-escape-newlines t) (print-escape-control-characters t))
    (when (getenv "REMACS_AUDIT_FORMS")
      (princ "FORM|") (prin1 id) (princ "|") (prin1 form) (terpri))
    (princ "AUDIT|") (prin1 id) (princ "|")
    (prin1 (condition-case e (list 'ok (eval form t)) (error (list 'error e))))
    (terpri))))

(audit-hash-run "hash:custom-test"
 '(progn
    (define-hash-table-test 'audit-case-fold
      (lambda (a b) (string= (downcase a) (downcase b)))
      (lambda (a) (sxhash-equal (downcase a))))
    (let ((h (make-hash-table :test 'audit-case-fold)))
      (puthash "HELLO" 42 h)
      (list (gethash "hello" h) (hash-table-test h)))))

(audit-hash-run "hash:cycle-lookup"
 '(let* ((a (list 1)) (b (list 1)) (h (make-hash-table :test 'equal)))
    (setcdr a a) (setcdr b b)
    (puthash a 42 h)
    (gethash b h)))

(audit-hash-run "equal:cycles"
 '(let ((a (list 1)) (b (list 1)))
    (setcdr a a) (setcdr b b) (equal a b)))

(audit-hash-run "equal:deep-different-lists"
 '(let ((a 1) (b 2))
    (dotimes (_ 205) (setq a (list a) b (list b)))
    (equal a b)))

(audit-hash-run "hash:equal-shared-subtree"
 '(let* ((a (list 1)) (b (list a a))
         (c (list (list 1) (list 1))) (h (make-hash-table :test 'equal)))
    (puthash b 42 h) (list (equal b c) (gethash c h))))

(audit-hash-run "hash:equal-byte-string"
 '(let ((h (make-hash-table :test 'equal))
        (a (unibyte-string 233)) (b (string 233)))
    (puthash a 42 h) (list (equal a b) (gethash b h))))

(audit-hash-run "hash:equal-bool-vector"
 '(let ((h (make-hash-table :test 'equal)))
    (puthash (make-bool-vector 3 t) 42 h)
    (gethash (make-bool-vector 3 t) h)))

(audit-hash-run "hash:equal-markers"
 '(with-temp-buffer
    (let ((h (make-hash-table :test 'equal)))
      (puthash (point-marker) 42 h) (gethash (point-marker) h))))

(audit-hash-run "hash:eql-zero"
 '(let ((h (make-hash-table :test 'eql)))
    (puthash 0.0 42 h) (list (eql 0.0 -0.0) (gethash -0.0 h))))

(audit-hash-run "hash:equal-zero"
 '(let ((h (make-hash-table :test 'equal)))
    (puthash 0.0 42 h) (list (equal 0.0 -0.0) (gethash -0.0 h))))

(audit-hash-run "hash:size-growth"
 '(let ((h (make-hash-table :size 1)))
    (dotimes (n 10) (puthash n n h))
    (list (hash-table-count h) (> (hash-table-size h) 1))))

(audit-hash-run "hash:weak-key-gc"
 '(let ((h (make-hash-table :test 'eq :weakness 'key)))
    (dotimes (n 1000) (puthash (vector n) 42 h))
    (garbage-collect) (garbage-collect)
    (< (hash-table-count h) 10)))

(princ "AUDIT-DONE\n")
