;;; -*- lexical-binding: nil; -*-
;; plist-put uses the live value cell and tail after predicate callbacks (#509).
(qa (let ((plist (list 'a 'old)))
      (plist-put plist 'a 'result
                 (lambda (&rest _) (setcdr plist (list 'replacement)) t))
      plist)
    '(a result))
(qa (let ((plist (list 'a 1 'b 'old)))
      (plist-put plist 'target 'result
                 (lambda (key prop)
                   (when (eq key 'a)
                     (setcdr (cdr plist) (list 'target 'old)))
                   (eq key prop)))
      plist)
    '(a 1 target result))
(princ (qa--done))
