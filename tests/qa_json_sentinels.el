;;; qa_json_sentinels.el --- Arbitrary JSON sentinel objects -*- lexical-binding: t; -*-
;; Check both serialization entry points; distinct equal objects retain their
;; ordinary JSON representation because sentinels are compared with eq.

(defun qa301-both (object &rest options)
  (list (apply #'json-serialize object options)
        (with-temp-buffer
          (apply #'json-insert object options)
          (buffer-string))))

(qa (qa301-both [t :null :false nil])
    '("[true,null,false,{}]" "[true,null,false,{}]"))

(qa (qa301-both [nil] :null-object nil)
    '("[null]" "[null]"))

(qa (qa301-both [nil] :false-object nil)
    '("[false]" "[false]"))

(qa (qa301-both [nil] :null-object nil :false-object nil)
    '("[null]" "[null]"))

(qa (qa301-both [t] :null-object t)
    '("[null]" "[null]"))

(qa (qa301-both [t] :false-object t)
    '("[false]" "[false]"))

(qa (qa301-both [42 43] :null-object 42)
    '("[null,43]" "[null,43]"))

(qa (qa301-both [42 43] :false-object 42)
    '("[false,43]" "[false,43]"))

(qa (qa301-both 42 :null-object 42)
    '("null" "null"))

(qa (qa301-both [sentinel] :null-object 'sentinel)
    '("[null]" "[null]"))

(qa (let ((x (copy-sequence "sentinel"))) (qa301-both (vector x (copy-sequence x)) :null-object x))
    '("[null,\"sentinel\"]" "[null,\"sentinel\"]"))

(qa (let ((x (copy-sequence "sentinel"))) (qa301-both (vector x (copy-sequence x)) :false-object x))
    '("[false,\"sentinel\"]" "[false,\"sentinel\"]"))

(qa (let ((x (list (cons 'x 1)))) (qa301-both (vector x (copy-tree x)) :null-object x))
    '("[null,{\"x\":1}]" "[null,{\"x\":1}]"))

(qa (let ((x (list (cons 'x 1)))) (qa301-both (vector x (copy-tree x)) :false-object x))
    '("[false,{\"x\":1}]" "[false,{\"x\":1}]"))

(qa (let ((x (vector 1))) (qa301-both (vector x (copy-sequence x)) :null-object x))
    '("[null,[1]]" "[null,[1]]"))

(qa (let ((x (vector 1))) (qa301-both (vector x (copy-sequence x)) :false-object x))
    '("[false,[1]]" "[false,[1]]"))

(qa (let ((x (make-hash-table))) (qa301-both (vector x) :null-object x))
    '("[null]" "[null]"))

(qa (let ((x (make-hash-table))) (qa301-both (vector x) :false-object x))
    '("[false]" "[false]"))

(qa (let ((x (string-to-number "1.5"))) (qa301-both (vector x (string-to-number "1.5")) :null-object x))
    '("[null,1.5]" "[null,1.5]"))

(qa (let ((x (current-buffer))) (qa301-both (vector x) :null-object x))
    '("[null]" "[null]"))

(qa (let ((x (make-marker))) (qa301-both (vector x) :false-object x))
    '("[false]" "[false]"))

(qa (let ((x (lambda () 1))) (qa301-both (vector x) :null-object x))
    '("[null]" "[null]"))

(qa (let ((x (make-bool-vector 2 t))) (qa301-both (vector x) :false-object x))
    '("[false]" "[false]"))

(qa (let ((x (list 1))) (qa301-both x :null-object x :false-object x))
    '("null" "null"))

(qa (let ((x (make-hash-table))) (qa301-both x :null-object x))
    '("null" "null"))

(qa (qa301-both (list :x nil) :null-object nil)
    '("{\"x\":null}" "{\"x\":null}"))

(qa (qa301-both (list (cons 'x nil)) :false-object nil)
    '("{\"x\":false}" "{\"x\":false}"))

(qa (let ((x (list 1)) (y (vector 2))) (qa301-both (json-parse-string "[null,false]" :null-object x :false-object y) :null-object x :false-object y))
    '("[null,false]" "[null,false]"))

(princ (qa--done))
