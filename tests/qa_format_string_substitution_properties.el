;;; -*- lexical-binding: nil; -*-
;; `format' merges format-spec and replacement text properties (#586).
(qa
 (let* ((fmt (propertize "[%s]" 'help-echo "from-format"))
        (arg (propertize "abc" 'audit-prop 'from-arg))
        (out (format fmt arg)))
   (list out
         (text-properties-at 0 out)
         (text-properties-at 1 out)))
 '(#("[abc]" 0 1 (help-echo "from-format")
           1 4 (audit-prop from-arg help-echo "from-format")
           4 5 (help-echo "from-format"))
   (help-echo "from-format")
   (audit-prop from-arg help-echo "from-format")))
(qa
 (let* ((fmt (propertize "%s" 'audit-prop 'from-format))
        (arg (propertize "x" 'audit-prop 'from-arg)))
   (get-text-property 0 'audit-prop (format fmt arg)))
 'from-arg)
(princ (qa--done))
