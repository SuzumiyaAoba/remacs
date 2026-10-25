;;; -*- lexical-binding: nil; -*-
;; plist-get and lax-plist-get return nil after an exhausted cycle (#503).
(let ((plist (list 'a 1 'b 2)))
  (setcdr (last plist) plist)
  (qa (plist-get plist 'missing) nil)
  (qa (plist-get plist 'b) 2)
  (qa (lax-plist-get plist 'missing) nil)
  (qa (plist-get plist 'missing #'eq) nil))
;; There is no arbitrary walk limit for a long finite plist.
(qa (plist-get (make-list 1000002 nil) 'missing) nil)
(princ (qa--done))
