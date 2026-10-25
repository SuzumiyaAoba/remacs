;;; -*- lexical-binding: nil; -*-
;; plist-member reports the GNU cycle tail and has no fixed walk cap (#504).
(qa (let ((plist (list 'a 1 'b 2)))
      (setcdr (last plist) plist)
      (condition-case err (plist-member plist 'missing)
        (circular-list
         (list (eq (cadr err) plist) (car-safe (cadr err))))))
    '(t a))
(qa (let ((plist (list 'a 1 'b 2)))
      (setcdr (last plist) plist)
      (eq (plist-member plist 'b) (cddr plist)))
    t)
(qa (plist-member (make-list 1000002 nil) 'missing) nil)
(princ (qa--done))
