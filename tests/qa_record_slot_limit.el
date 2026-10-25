;;; -*- lexical-binding: nil; -*-
;; Record constructors enforce GNU's 4095 total-slot limit (#597).
(qa (length (make-record 'x 4094 nil)) 4095)
(qa
 (condition-case err
     (make-record 'x most-positive-fixnum nil)
   (error (error-message-string err)))
 "Attempt to allocate a record of 2305843009213693952 slots; max is 4095")
(qa
 (condition-case err
     (make-record 'x 4095 nil)
   (error (error-message-string err)))
 "Attempt to allocate a record of 4096 slots; max is 4095")
(qa
 (length (apply #'record (cons 'x (make-list 4094 nil))))
 4095)
(qa
 (condition-case err
     (apply #'record (cons 'x (make-list 4095 nil)))
   (error (error-message-string err)))
 "Attempt to allocate a record of 4096 slots; max is 4095")
(princ (qa--done))
