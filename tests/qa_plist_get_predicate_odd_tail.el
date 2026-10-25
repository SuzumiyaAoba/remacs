;;; -*- lexical-binding: nil; -*-
;; plist-get does not call PREDICATE for an unmatched final key (#505).
(qa (let ((calls 0))
      (list (plist-get '(a) 'a (lambda (&rest _) (setq calls (1+ calls)) t))
            calls))
    '(nil 0))
(qa (plist-get '(a) 'a 5) nil)
(qa (plist-get '(a nil) 'a #'eq) nil)
(princ (qa--done))
