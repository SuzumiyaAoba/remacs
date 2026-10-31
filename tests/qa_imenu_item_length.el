;;; -*- lexical-binding: t -*-
(require 'imenu)

(defun qa-imenu--limited-items (limit items)
  (let ((imenu-max-item-length limit))
    (imenu--truncate-items items)
    items))

(qa (qa-imenu--limited-items nil (list (cons "example" 1))) '(("example" . 1)))
(qa (qa-imenu--limited-items 60 (list (cons "example" 1))) '(("example" . 1)))
(qa (qa-imenu--limited-items 7 (list (cons "example" 1))) '(("example" . 1)))
(qa (qa-imenu--limited-items 3 (list (cons "example" 1))) '(("exa" . 1)))
(qa (qa-imenu--limited-items 0 (list (cons "example" 1))) '(("" . 1)))
(qa (qa-imenu--limited-items 3 (list (list "Section" (cons "longer" 2))))
    '(("Sec" ("lon" . 2))))
(qa (qa-imenu--limited-items 3 (list (cons "日本語の名前" 1))) '(("日本語" . 1)))
(qa-err (qa-imenu--limited-items -1 (list (cons "example" 1))) 'user-error)
(qa-err (qa-imenu--limited-items -99 (list (cons "example" 1))) 'user-error)
(qa-err (qa-imenu--limited-items 1.5 (list (cons "example" 1))) 'user-error)
(qa-err (qa-imenu--limited-items "3" (list (cons "example" 1))) 'user-error)
(qa (let* ((items (list (cons "example" 1) (cons "another" 2)))
           (before (copy-tree items)))
      (condition-case nil
          (qa-imenu--limited-items -1 items)
        (user-error nil))
      (equal items before))
    t)

(princ (qa--done))
