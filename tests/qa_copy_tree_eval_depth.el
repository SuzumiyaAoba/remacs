;;; -*- lexical-binding: nil; -*-
;; copy-tree honors recursive Lisp depth without limiting flat spines (#533).
(qa-err (let ((tree 'leaf) (max-lisp-eval-depth 100))
          (dotimes (_ 300) (setq tree (list tree)))
          (copy-tree tree))
        'excessive-lisp-nesting)
(qa (let* ((tree (make-list 500001 nil))
          (copy (copy-tree tree)))
      (list (= (length copy) 500001) (eq tree copy)))
    '(t nil))
(princ (qa--done))
