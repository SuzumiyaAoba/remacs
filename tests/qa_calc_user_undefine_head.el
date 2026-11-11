;;; -*- lexical-binding: t; -*-
;; Removing a user key must update the z-prefix map's head (#1992).

(require 'calc-prog)

(defun qa-calc-user-undefine-map (keys key)
  (let ((calc-mode-map (make-sparse-keymap))
        (user-map (make-sparse-keymap)))
    (define-key calc-mode-map "z" user-map)
    (dolist (binding keys)
      (define-key user-map (vector binding) #'ignore))
    (cl-letf (((symbol-function 'read-char)
               (lambda (&rest _) key)))
      (calc-user-undefine))
    (mapcar (lambda (binding)
              (lookup-key calc-mode-map (vector ?z binding)))
            keys)))

(qa
 (list (qa-calc-user-undefine-map '(?a ?b) ?a)
       (qa-calc-user-undefine-map '(?a ?b) ?b))
 '((nil ignore) (ignore nil)))

(princ (qa--done))
