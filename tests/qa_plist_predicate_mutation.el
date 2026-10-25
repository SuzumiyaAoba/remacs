;;; -*- lexical-binding: nil; -*-
;; plist-get/member reread the live cdr after predicate callbacks (#506).
(qa (let ((plist (list 'a 'old)))
      (plist-get plist 'a
                 (lambda (&rest _) (setcdr plist (list 'new)) t)))
    'new)
(qa (let ((plist (list 'a 1 'b 'old)))
      (plist-get plist 'target
                 (lambda (key target)
                   (when (eq key 'a) (setcdr (cdr plist) (list 'target 'new)))
                   (eq key target))))
    'new)
(qa (let ((plist (list 'a 1 'b 'old))
          (replacement (list 'target 'new)))
      (eq (plist-member plist 'target
                        (lambda (key target)
                          (when (eq key 'a) (setcdr (cdr plist) replacement))
                          (eq key target)))
          replacement))
    t)
(princ (qa--done))
