;;; -*- lexical-binding: nil; -*-
;; The hash index follows allocated capacity, not current occupancy (#539).
(qa
 (mapcar (lambda (size)
           (internal--hash-table-index-size (make-hash-table :size size)))
         '(13 14 26 27 52 53 104 105 100))
 '(16 16 32 32 64 64 128 128 128))

(qa
 (let ((table (make-hash-table :size 0)))
   (let ((empty (internal--hash-table-index-size table)))
     (puthash 'key 'value table)
     (let ((allocated (internal--hash-table-index-size table)))
       (remhash 'key table)
       (let ((after-delete (internal--hash-table-index-size table)))
         (clrhash table)
         (list empty allocated after-delete
               (internal--hash-table-index-size table)
               (internal--hash-table-index-size (copy-hash-table table)))))))
 '(1 8 8 8 8))

(qa
 (let ((table (make-hash-table :size 100)))
   (list (internal--hash-table-index-size table)
         (progn (puthash 'key 'value table)
                (internal--hash-table-index-size table))
         (progn (clrhash table)
                (internal--hash-table-index-size table))))
 '(128 128 128))
(princ (qa--done))
