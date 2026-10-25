;;; -*- lexical-binding: nil; -*-
;; value< orders markers, buffers and processes by GNU's reference rules (#566).
(qa (with-temp-buffer
      (insert "ab")
      (let ((a (copy-marker 1)) (b (copy-marker 2)))
        (value< a b)))
    t)

(qa (let ((detached-a (make-marker))
          (detached-b (make-marker)))
      (with-temp-buffer
        (insert "x")
        (let ((live (copy-marker 1)))
          (list (value< detached-a live)
                (value< live detached-a)
                (value< detached-a detached-b)))))
    '(t nil nil))

(qa
 (let ((a (get-buffer-create "qa-value-buffer-a"))
       (b (get-buffer-create "qa-value-buffer-b")))
   (unwind-protect
       (list (value< a b) (value< b a))
     (kill-buffer a)
     (kill-buffer b)))
 '(t nil))

(qa
 (let ((dead (get-buffer-create "qa-value-buffer-dead"))
       (live (get-buffer-create "qa-value-buffer-live")))
   (kill-buffer dead)
   (unwind-protect (value< dead live) (kill-buffer live)))
 t)

(qa
 (let ((a (make-pipe-process :name "qa-value-proc-a" :noquery t))
       (b (make-pipe-process :name "qa-value-proc-b" :noquery t)))
   (unwind-protect (list (value< a b) (value< b a))
     (delete-process a)
     (delete-process b)))
 '(t nil))
(princ (qa--done))
