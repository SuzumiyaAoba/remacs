;;; -*- lexical-binding: nil; -*-
;; %-spec text properties override substitution properties (#572).
(require 'format-spec)
(qa
 (let* ((format (propertize "%a" 'face 'italic))
        (substitution (propertize "foo" 'face 'bold))
        (result (format-spec format (list (cons ?a substitution)))))
   (list result (text-properties-at 0 result)))
 '(#("foo" 0 3 (face italic)) (face italic)))
(qa
 (let* ((format (propertize "%a" 'help-echo "from-format"))
        (substitution (propertize "foo" 'help-echo "from-sub"))
        (result (format-spec format (list (cons ?a substitution)))))
   (list result (get-text-property 0 'help-echo result)))
 '(#("foo" 0 3 (help-echo "from-format")) "from-format"))
(princ (qa--done))
