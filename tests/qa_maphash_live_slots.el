;;; -*- lexical-binding: nil; -*-
;; maphash reads each live slot just before invoking its callback (#537).
(qa
 (let ((table (make-hash-table :test 'eq)) seen)
   (puthash 'a 1 table) (puthash 'b 2 table) (puthash 'c 3 table)
   (maphash (lambda (key value)
              (push (cons key value) seen)
              (when (eq key 'a) (puthash 'b 20 table)))
            table)
   (nreverse seen))
 '((a . 1) (b . 20) (c . 3)))

(qa
 (let ((table (make-hash-table :test 'eq)) seen)
   (puthash 'a 1 table) (puthash 'b 2 table) (puthash 'c 3 table)
   (maphash (lambda (key _value)
              (push key seen)
              (when (eq key 'a) (remhash 'b table)))
            table)
   (nreverse seen))
 '(a c))

(qa
 (let ((table (make-hash-table :test 'eq)) seen)
   (puthash 'a 1 table) (puthash 'b 2 table) (puthash 'c 3 table)
   (maphash (lambda (key _value)
              (push key seen)
              (when (eq key 'a) (clrhash table)))
            table)
   (nreverse seen))
 '(a))

(qa
 (let ((table (make-hash-table :test 'eq)) seen)
   (puthash 'a 1 table) (puthash 'b 2 table) (puthash 'c 3 table)
   (maphash (lambda (key _value)
              (push key seen)
              (when (eq key 'a)
                (remhash 'b table)
                (puthash 'x 20 table)))
            table)
   (nreverse seen))
 '(a x c))

(qa
 (let ((table (make-hash-table :test 'eq)) outer inner)
   (puthash 'a 1 table) (puthash 'b 2 table) (puthash 'c 3 table)
   (maphash (lambda (key _value)
              (push key outer)
              (when (eq key 'a)
                (maphash (lambda (nested-key _nested-value)
                           (push nested-key inner))
                         table)))
            table)
   (list (nreverse outer) (nreverse inner)))
 '((a b c) (a b c)))
(princ (qa--done))
