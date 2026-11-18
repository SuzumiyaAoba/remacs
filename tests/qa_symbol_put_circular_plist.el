;;; -*- lexical-binding: nil; -*-
;; `put' reports the complete circular symbol plist in its error data.
(qa
 (let* ((symbol (make-symbol "qa-symbol-put-cycle"))
        (plist (list 'first 1 'second 2))
        (tail (last plist)))
   (setcdr tail plist)
   (unwind-protect
       (progn
         (setplist symbol plist)
         (condition-case err
             (put symbol 'missing 42)
           (circular-list (list (eq (cadr err) plist)
                                (car (cadr err))))))
     (setplist symbol nil)
     (setcdr tail nil)))
 '(t first))
(princ (qa--done))
