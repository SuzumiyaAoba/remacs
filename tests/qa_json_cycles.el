;;; qa_json_cycles.el --- JSON recursion and circular list guards -*- lexical-binding: t; -*-
;; Depth errors are catchable; shared values remain valid and failed insertions are atomic.

(defun qa314-error (error root)
  (cond ((eq (car error) 'circular-list)
         (let* ((datum (cadr error)) (head (car datum)))
           (list 'circular-list (eq datum root) (if (consp head) (car head) head))))
        ((eq (car error) 'wrong-type-argument)
         (list (car error) (cadr error) (eq (caddr error) root)))
        (t error)))

(defun qa314-both (value &rest options)
  (list
   (condition-case error
       (list 'ok (length (apply #'json-serialize value options)))
     (error (qa314-error error value)))
   (with-temp-buffer
     (insert "seed") (goto-char 2)
     (let ((result (condition-case error
                       (progn (apply #'json-insert value options) 'inserted)
                     (error (qa314-error error value)))))
       (list result (point) (buffer-size) (equal (buffer-string) "seed"))))))

(defun qa314-depth (depth base &rest options)
  (let ((value base))
    (dotimes (_ depth) (setq value (vector value)))
    (apply #'qa314-both value options)))

;; self vector
(qa (let ((value (vector nil))) (aset value 0 value) (qa314-both value))
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; self hash
(qa (let ((value (make-hash-table :test 'equal))) (puthash "x" value value) (qa314-both value))
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; recursive alist value
(qa (let ((value (list (cons 'x nil)))) (setcdr (car value) value) (qa314-both value))
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; circular alist tail
(qa (let ((value (list (cons 'x 1)))) (setcdr value value) (qa314-both value))
    '((circular-list t x) ((circular-list t x) 2 4 t)))

;; circular plist tail
(qa (let ((value (list :x 1))) (setcdr (cdr value) value) (qa314-both value))
    '((circular-list t :x) ((circular-list t :x) 2 4 t)))

;; prefixed alist cycle datum
(qa (let ((value (list (cons 'a 1) (cons 'b 2) (cons 'c 3)))) (setcdr (last value) (cdr value)) (qa314-both value))
    '((circular-list nil c) ((circular-list nil c) 2 4 t)))

;; prefixed plist cycle datum
(qa (let ((value (list :a 1 :b 2 :c 3))) (setcdr (last value) (cddr value)) (qa314-both value))
    '((circular-list nil :c) ((circular-list nil :c) 2 4 t)))

;; 49 wrappers with scalar
(qa (qa314-depth 49 0)
    '((ok 99) (inserted 101 103 nil)))

;; 49 wrappers with empty object
(qa (qa314-depth 49 nil)
    '((ok 100) (inserted 102 104 nil)))

;; 49 wrappers with empty vector
(qa (qa314-depth 49 [])
    '((ok 100) (inserted 102 104 nil)))

;; 50 wrappers with scalar
(qa (qa314-depth 50 0)
    '((ok 101) (inserted 103 105 nil)))

;; 50 wrappers with empty object
(qa (qa314-depth 50 nil)
    '((ok 102) (inserted 104 106 nil)))

;; 50 wrappers with empty vector
(qa (qa314-depth 50 [])
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; 51 wrappers with scalar
(qa (qa314-depth 51 0)
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; 51 wrappers with empty object
(qa (qa314-depth 51 nil)
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; 51 wrappers with empty vector
(qa (qa314-depth 51 [])
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; 50 wrappers empty hash
(qa (qa314-depth 50 (make-hash-table))
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; 49 wrappers nonempty plist
(qa (qa314-depth 49 '(:x 1))
    '((ok 105) (inserted 107 109 nil)))

;; 50 wrappers nonempty plist
(qa (qa314-depth 50 '(:x 1))
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; very deep input terminates
(qa (qa314-depth 1200 0)
    '((error "Maximum JSON serialization depth exceeded") ((error "Maximum JSON serialization depth exceeded") 2 4 t)))

;; shared vector is not a cycle
(qa (let ((shared (vector 1))) (json-serialize (vector shared shared)))
    '"[[1],[1]]")

;; shared alist is not a cycle
(qa (let ((shared (list (cons 'x 1)))) (json-serialize (vector shared shared)))
    '"[{\"x\":1},{\"x\":1}]")

;; duplicate key skips cyclic value
(qa (let ((cycle (vector nil))) (aset cycle 0 cycle) (json-serialize (list :x 1 :x cycle)))
    '"{\"x\":1}")

;; cyclic sentinel checked first
(qa (let ((cycle (vector nil))) (aset cycle 0 cycle) (qa314-both cycle :null-object cycle))
    '((ok 4) (inserted 6 8 nil)))

;; sentinel at depth boundary
(qa (let ((sentinel (vector 1))) (qa314-depth 50 sentinel :false-object sentinel))
    '((ok 105) (inserted 107 109 nil)))

;; dotted list remains type error
(qa (qa314-both (cons (cons 'x 1) 'tail))
    '((wrong-type-argument listp t) ((wrong-type-argument listp t) 2 4 t)))

(princ (qa--done))
