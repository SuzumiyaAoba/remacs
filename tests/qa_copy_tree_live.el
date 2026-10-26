;;; -*- lexical-binding: nil; -*-
;; Recursive calls use the live copy-tree cell, including advice (#534).
(let ((seen nil) (options nil) (token (list 'option)))
  (advice-add 'copy-tree :before
              (lambda (input &optional option)
                (push input seen)
                (push option options))
              '((name . qa-copy-tree-before)))
  (unwind-protect
      (progn
        (setq seen nil options nil)
        (qa (copy-tree '((a) (b))) '((a) (b)))
        (qa seen '((b) (a) ((a) (b))))
        (qa options '(nil nil nil))
        (setq seen nil options nil)
        (qa (copy-tree [a b] token) [a b])
        (qa seen '(a b [a b]))
        (qa (length options) 3)
        (qa (eq (car options) token) t)
        (qa (eq (cadr options) token) t)
        (qa (eq (caddr options) token) t)
        (let ((source (record 'node 'a)))
          (setq seen nil options nil)
          (qa (copy-tree source token) source)
          (qa seen (list 'node 'a source)))
        (setq seen nil options nil)
        (qa (copy-tree '(head . [a]) token) '(head . [a]))
        (qa seen '(a [a] (head . [a]))))
    (advice-remove 'copy-tree 'qa-copy-tree-before)))
;; Calling a saved definition still resolves its recursive calls by name.
(let ((saved (symbol-function 'copy-tree))
      (seen nil) (options nil) (token (list 'option)))
  (unwind-protect
      (progn
        (fset 'copy-tree (lambda (input &optional option)
                            (push input seen)
                            (push option options)
                            'replacement))
        (qa (funcall saved '((a) (b)) token)
            '(replacement replacement))
        (qa seen '((b) (a)))
        (qa (eq (car options) token) t)
        (qa (funcall saved [a b] token) [replacement replacement])
        (qa (funcall saved '(head . [a]) token) '(head . replacement))
        ;; The recursive call can change the enclosing list's cdr.
        (let* ((source (list (list 'a) 'old)) (trigger (car source)))
          (fset 'copy-tree
                (lambda (input &optional _option)
                  (when (eq input trigger) (setcdr source '(new)))
                  'copied))
          (qa (funcall saved source) '(copied new))
          (qa source '((a) new)))
        ;; Propagate failures from the live recursive function.
        (fset 'copy-tree (lambda (&rest _) (error "recursive callback")))
        (qa (condition-case e (funcall saved '((a)))
              (error (cadr e)))
            "recursive callback"))
    (fset 'copy-tree saved)))
(princ (qa--done))
