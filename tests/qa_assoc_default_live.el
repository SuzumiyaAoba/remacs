;;; -*- lexical-binding: nil; -*-
;; assoc-default calls live TEST/equal and follows live cdrs (#525-#527).
(let* ((value (list 'result)) (new-tail (list (cons 'target value)))
       (source (list (cons 'first 'old))) (calls 0))
  (qa (eq (assoc-default 'target source
                          (lambda (key wanted)
                            (setq calls (1+ calls))
                            (when (= calls 1) (setcdr source new-tail))
                            (eq key wanted))) value) t)
  (qa calls 2))
(let ((source (list (cons 'a 'old) (cons 'target 'result))) (calls 0))
  (qa (assoc-default 'target source
                     (lambda (key wanted) (setq calls (1+ calls)) (setcdr source nil) nil)) nil)
  (qa calls 1))
(let* ((element (cons 'a 'old)) (source (list element)) (value (list 'new)))
  (qa (eq (assoc-default 'a source
                          (lambda (&rest args) (setcdr element value) t)) value) t))
(let* ((element (cons 'a 'kept)) (source (list element)))
  (qa (assoc-default 'a source
                     (lambda (&rest args) (setcar source (cons 'a 'replacement)) t)) 'kept))
(let ((source (list (cons 'key 'result))) (calls 0))
  (setcdr source source)
  (qa (assoc-default 'absent source
                     (lambda (&rest args) (setq calls (1+ calls)) (> calls 500000))) 'result)
  (qa calls 500001))
(let ((source (list (cons 'key 'result))) (calls 0))
  (setcdr source source)
  (qa (assoc-default 'absent source
                     (lambda (&rest args)
                       (setq calls (1+ calls))
                       (when (= calls 10) (setcdr source nil))
                       nil)) nil)
  (qa calls 10))
(let ((fallback (list 'default)) (calls 0))
  (qa (eq (assoc-default 'target '(target (target . later))
                          (lambda (element wanted) (setq calls (1+ calls)) (eq element wanted))
                          fallback) fallback) t)
  (qa calls 1))
(qa (assoc-default 'x nil 5) nil)
(qa-err (assoc-default 'x '(x) 5) 'invalid-function)
(qa-err-data (assoc-default 'x 'atom 5) 'wrong-type-argument '(listp atom))
(qa-err-data (assoc-default 'x '(a . tail)) 'wrong-type-argument '(listp tail))
;; Restore equal before using qa: its assertions must use real equality.
(let ((saved (symbol-function 'equal)) (result nil) (seen nil))
  (unwind-protect
      (progn
        (fset 'equal (lambda (element wanted) (setq seen (list element wanted)) t))
        (setq result (assoc-default 'wanted '((unrelated . value)))))
    (fset 'equal saved))
  (qa result 'value)
  (qa seen '(unrelated wanted)))
(let* ((source (make-list 500000 nil)) (end (last source))
       (value (list 'result)) (tail (list (cons 'target value))))
  (setcdr end tail)
  (unwind-protect
      (progn
        (qa (eq (assoc-default 'target source) value) t)
        (qa (eq (assoc-default 'target source #'eq) value) t))
    (setcdr end source)))
(garbage-collect)
(princ (qa--done))
