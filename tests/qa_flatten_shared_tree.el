;;; -*- lexical-binding: nil; -*-
;; Visits to shared, finite subtrees have no arbitrary cutoff (#462).
(let ((tree nil))
  (dotimes (depth 20) (setq tree (cons tree tree)))
  (qa (flatten-tree (cons tree 'sentinel)) '(sentinel))
  (qa (flatten-tree (list 'before tree 'after)) '(before after))
  (qa (flatten-list (cons tree 'alias-sentinel)) '(alias-sentinel)))
;; Sharing is not a reason to suppress repeated leaves.
(let* ((leaf (propertize "leaf" 'tag 'value))
       (branch (cons leaf (vector 'opaque)))
       (tree (list branch branch)))
  (qa (flatten-tree tree) (list leaf (cdr branch) leaf (cdr branch)))
  (qa (eq (car (flatten-tree tree)) leaf) t)
  (qa (eq (nth 1 (flatten-tree tree)) (cdr branch)) t)
  (qa (eq (car tree) (cadr tree)) t))
;; Deep car nesting is traversed without recursive native stack growth.
(let ((tree 'bottom))
  (dotimes (depth 10000) (setq tree (list tree)))
  (qa (flatten-tree tree) '(bottom)))
(qa (flatten-tree '(a nil (b . c) [d e])) '(a b c [d e]))
(princ (qa--done))
