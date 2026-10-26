;;; -*- lexical-binding: nil; -*-
;; GNU's 4095 total record slots include the type slot (#597).
(dolist (size '(0 1 4093 4094))
  (let* ((payload (list 'shared))
         (record (make-record 'qa597 size payload)))
    (qa (length record) (1+ size))
    (qa (aref record 0) 'qa597)
    (when (> size 0)
      (qa (eq (aref record size) payload) t)))
  (qa (length (apply #'record 'qa597 (make-list size nil))) (1+ size))
  (qa (length (read (concat "#s(qa597" (make-string size ?\s) ")"))) 1)
  (qa (length (read (concat "#s(qa597 "
                           (mapconcat (lambda (_) "nil") (make-list size nil) " ")
                           ")"))) (1+ size)))
(dolist (entry '((4095 "Attempt to allocate a record of 4096 slots; max is 4095")
                 (4096 "Attempt to allocate a record of 4097 slots; max is 4095")))
  (let ((size (car entry)) (message (cadr entry)))
    (qa-err-data (make-record 'qa597 size nil) 'error (list message))
    (qa-err-data (apply #'record 'qa597 (make-list size nil)) 'error (list message))
    (qa-err-data (read (concat "#s(qa597 "
                              (mapconcat (lambda (_) "nil") (make-list size nil) " ")
                              ")")) 'error (list message))))
(qa-err-data (make-record 'qa597 most-positive-fixnum nil) 'error
             '("Attempt to allocate a record of 2305843009213693952 slots; max is 4095"))
(dolist (size '(nil -1 1.0 2305843009213693952 10000000000000000000000000000000000000000000000))
  (qa-err-data (make-record 'qa597 size nil) 'wrong-type-argument (list 'wholenump size)))
;; TYPE is unrestricted; rejected sizes must not impose type validation.
(dolist (type '(nil 1 "type" [tag]))
  (qa (eq (aref (make-record type 0 nil) 0) type) t)
  (qa-err-data (make-record type 4095 nil) 'error
               '("Attempt to allocate a record of 4096 slots; max is 4095")))
;; This limit is specific to records, not ordinary vectors or bool-vectors.
(qa (length (make-vector 4096 nil)) 4096)
(qa (length (make-bool-vector 4096 nil)) 4096)
(princ (qa--done))
