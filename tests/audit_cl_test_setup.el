;;; audit_cl_test_setup.el --- test prerequisites -*- lexical-binding: t; -*-

(load (expand-file-name "tests/audit_helpers.el") nil t)

(audit-check "test-setup:cl_defstruct_accepts_docstring"
 '(progn (when (getenv "AUDIT_REQUIRE_CL") (require 'cl-lib))
(progn (cl-defstruct ec-probe-struct "A struct doc." one (two 2))
                  (let ((s (make-ec-probe-struct :one 1)))
                    (list (ec-probe-struct-one s)
                          (ec-probe-struct-two s)
                          (ec-probe-struct-p s))))))

(audit-check "test-setup:cl_destructuring_bind_dotted_tail"
 '(progn (when (getenv "AUDIT_REQUIRE_CL") (require 'cl-lib))
(list (cl-destructuring-bind (a . b) '(1 2 3) (list a b))
                 (cl-destructuring-bind (a b . c) '(1 2 3 4) (list a b c)))))

(audit-check "test-setup:cl_flet_lexical_escape"
 '(progn (when (getenv "AUDIT_REQUIRE_CL") (require 'cl-lib))
(eval '(let ((f (cl-flet ((h (x) (* x 2)))
                             (lambda (y) (h y)))))
                   (funcall f 5)) t)))

(audit-check "test-setup:cl_labels_recursion"
 '(progn (when (getenv "AUDIT_REQUIRE_CL") (require 'cl-lib))
(eval '(list (cl-labels ((f (n) (if (<= n 1) 1 (* n (f (1- n))))))
                          (f 5))
                        (cl-labels ((ev (n) (if (= n 0) t (od (1- n))))
                                    (od (n) (if (= n 0) nil (ev (1- n)))))
                          (ev 10))
                        (let ((f (cl-labels ((g (n) (if (<= n 0) 0 (+ n (g (1- n))))))
                                   (lambda (k) (g k)))))
                          (funcall f 4))) t)))

(audit-check "test-setup:cl_loop_hash_iteration"
 '(progn (when (getenv "AUDIT_REQUIRE_CL") (require 'cl-lib))
(let ((h (make-hash-table :test 'equal)))
              (puthash "a" 1 h)
              (puthash "b" 2 h)
              (list (sort (cl-loop for k being the hash-keys of h
                                   using (hash-values v)
                                   collect (cons k v))
                          (lambda (a b) (string< (car a) (car b))))
                    (sort (cl-loop for v being the hash-values of h
                                   using (hash-keys k)
                                   collect (cons v k))
                          (lambda (a b) (< (car a) (car b))))))))

(princ "AUDIT-DONE\n")
