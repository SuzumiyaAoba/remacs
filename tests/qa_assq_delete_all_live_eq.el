;;; -*- lexical-binding: nil; -*-
;; assq-delete-all delegates through live assoc-delete-all and eq cells (#529).
(qa (let ((saved (symbol-function 'eq)) result)
      (unwind-protect
          (progn
            (fset 'eq (lambda (&rest _) t))
            (setq result (assq-delete-all 'target (list (cons 'other 'value))))
            result)
        (fset 'eq saved)))
    nil)
(qa (let ((saved (symbol-function 'assoc-delete-all))
          (alist (list (cons 'a 1))) result)
      (unwind-protect
          (progn
            (fset 'assoc-delete-all (lambda (&rest args) args))
            (setq result (assq-delete-all 'key alist)))
        (fset 'assoc-delete-all saved))
      result)
    '(key ((a . 1)) eq))
(princ (qa--done))
