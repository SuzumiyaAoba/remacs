;;; -*- lexical-binding: nil; -*-
;; process-attributes validates numeric PIDs without narrowing or accepting processes (#608).
(qa
 (consp (car (process-attributes (float (emacs-pid)))))
 t)
(qa
 (mapcar (lambda (pid)
           (condition-case err
               (process-attributes pid)
             (error (error-message-string err))))
         '(1.5 2147483648 4294967297 2305843009213693952))
 '("Not an in-range integer, integral float, or cons of integers"
   "Not an in-range integer, integral float, or cons of integers"
   "Not an in-range integer, integral float, or cons of integers"
   "Not an in-range integer, integral float, or cons of integers"))
(qa
 (let ((proc (make-pipe-process :name "qa-process-attributes-pid" :buffer nil)))
   (unwind-protect
       (condition-case err
           (process-attributes proc)
         (error (car err)))
     (delete-process proc)))
 'wrong-type-argument)
(princ (qa--done))
