;; overlays — GNU 31.1 pins (verified): make-overlay/overlay-start/end/
;; buffer/get/put/delete/move/copy + overlays-at/in, next/previous-
;; overlay-change, overlay-lists, delete-all-overlays.
;; NOTE: `qa' evals FORMs dynamically — `let' must be INSIDE each form.
;;
;; GNU truths pinned:
;;   - make-overlay with BEG > END swaps them.
;;   - 3rd arg of make-overlay is the BUFFER.
;;   - delete-overlay detaches: overlay-buffer/start/end → nil; the
;;     overlay object still satisfies overlayp.
;;   - Default insertion semantics: text at the START is included in the
;;     overlay (start stays), text at the END is excluded (end stays);
;;     with front/rear-advance t,t both endpoints move.
;;   - overlays-at POS covers beg <= POS < end; empty (beg=end) overlays
;;     are NOT reported by overlays-at but ARE by overlays-in.
;;   - overlays-in intersects [beg,end); reversed args → nil.
;;   - overlay-put PROP nil leaves the prop present with nil value
;;     (overlay-properties still lists it).
;;   - overlay-properties returns a PLIST.
;;   - copy-overlay takes 1 arg in 31.1 (copies into the same buffer).
;;   - move-overlay can retarget to another buffer (4th arg).

;; == make-overlay / overlayp / accessors ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o (make-overlay 2 5)))
          (prog1 (list (overlayp o) (overlay-start o) (overlay-end o)
                       (eq (overlay-buffer o) buf))
            (kill-buffer buf)))))
    '(t 2 5 t))
;; beg > end swaps
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abc")
        (let ((o (make-overlay 3 1)))
          (prog1 (list (overlay-start o) (overlay-end o))
            (kill-buffer buf)))))
    '(1 3))
;; BUFFER arg
(qa (let ((b1 (generate-new-buffer " *x*"))
          (b2 (generate-new-buffer " *y*")))
      (unwind-protect
          (with-current-buffer b1
            (eq (overlay-buffer (make-overlay 1 1 b2)) b2))
        (kill-buffer b1) (kill-buffer b2)))
    t)
(qa (overlayp 'x) nil)
(qa (overlayp [1]) nil)
(qa (overlayp nil) nil)
(qa-err (make-overlay "a" 2) 'wrong-type-argument)
(qa-err (overlay-start 'x) 'wrong-type-argument)
(qa-err (move-overlay 'x 1 2) 'wrong-type-argument)
(qa-err (delete-overlay 'x) 'wrong-type-argument)

;; == overlay-put / overlay-get ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdef")
        (let ((o (make-overlay 1 3)))
          (prog1 (list (overlay-get o 'face)
                       (overlay-put o 'face 'bold)
                       (overlay-get o 'face)
                       (overlay-put o 'priority 5)
                       (overlay-get o 'priority))
            (kill-buffer buf)))))
    '(nil bold bold 5 5))
;; put nil leaves the property (value nil)
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdef")
        (let ((o (make-overlay 1 3)))
          (overlay-put o 'a 1)
          (overlay-put o 'a nil)
          (prog1 (list (overlay-get o 'a) (overlay-properties o))
            (kill-buffer buf)))))
    '(nil (a nil)))
;; overlay-properties is a plist
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdef")
        (let ((o (make-overlay 1 3)))
          (overlay-put o 'a 1)
          (overlay-put o 'b 2)
          (let ((pl (overlay-properties o)))
            (prog1 (list (plist-get pl 'a) (plist-get pl 'b) (length pl))
              (kill-buffer buf))))))
    '(1 2 4))
(qa-err (overlay-get nil 'face) 'wrong-type-argument)

;; == move-overlay ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o (make-overlay 2 5)))
          (move-overlay o 3 7)
          (prog1 (list (overlayp (move-overlay o 3 7))
                       (overlay-start o) (overlay-end o))
            (kill-buffer buf)))))
    '(t 3 7))
;; move to another buffer
(qa (let ((b1 (generate-new-buffer " *x*"))
          (b2 (generate-new-buffer " *y*")))
      (unwind-protect
          (progn
            (with-current-buffer b1 (insert "aaaaaa"))
            (with-current-buffer b2 (insert "zzzzzz"))
            (let ((o (with-current-buffer b1 (make-overlay 1 3))))
              (move-overlay o 2 4 b2)
              (list (eq (overlay-buffer o) b2) (overlay-start o))))
        (kill-buffer b1) (kill-buffer b2)))
    '(t 2))

;; == delete-overlay detaches ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdef")
        (let ((o (make-overlay 1 3)))
          (delete-overlay o)
          (prog1 (list (overlayp o) (overlay-buffer o) (overlay-start o)
                       (overlay-end o))
            (kill-buffer buf)))))
    '(t nil nil nil))
;; deleting twice is a no-op
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abc")
        (let ((o (make-overlay 1 2)))
          (delete-overlay o)
          (prog1 (delete-overlay o)
            (kill-buffer buf)))))
    nil)

;; == copy-overlay (single arg in 31.1) ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let* ((o (make-overlay 2 5))
               (c (progn (overlay-put o 'face 'italic) (copy-overlay o))))
          (prog1 (list (overlayp c) (eq (overlay-buffer c) buf)
                       (overlay-start c) (overlay-end c)
                       (overlay-get c 'face))
            (kill-buffer buf)))))
    '(t t 2 5 italic))
;; mutating the copy doesn't touch the original
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let* ((o (make-overlay 2 5))
               (c (copy-overlay o)))
          (overlay-put c 'priority 9)
          (prog1 (list (overlay-get o 'priority) (overlay-get c 'priority))
            (kill-buffer buf)))))
    '(nil 9))

;; == insertion advance semantics ==
;; default: text at START included (start stays), text at END excluded
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o (make-overlay 3 6)))
          (goto-char 3)
          (insert "XX")
          (let ((s (overlay-start o)) (e (overlay-end o)))
            (goto-char e)
            (insert "YY")
            (prog1 (list s e (overlay-start o) (overlay-end o))
              (kill-buffer buf))))))
    '(3 8 3 8))
;; front/rear-advance t: both endpoints move
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o (make-overlay 3 6 nil t t)))
          (goto-char 3)
          (insert "XX")
          (let ((s (overlay-start o)) (e (overlay-end o)))
            (goto-char e)
            (insert "YY")
            (prog1 (list s e (overlay-start o) (overlay-end o))
              (kill-buffer buf))))))
    '(5 8 5 10))

;; == overlays-at ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o1 (make-overlay 2 6))
              (o2 (make-overlay 4 8)))
          (prog1 (list (length (overlays-at 3))
                       (length (overlays-at 1))
                       (length (overlays-at 10))
                       (not (null (memq o1 (overlays-at 4)))))
            (kill-buffer buf)))))
    '(1 0 0 t))
;; half-open [beg,end): at beg yes, at end no
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o (make-overlay 3 6)))
          (prog1 (list (length (overlays-at 3))
                       (length (overlays-at 5))
                       (length (overlays-at 6)))
            (kill-buffer buf)))))
    '(1 1 0))
;; empty overlay not reported by overlays-at
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abc")
        (make-overlay 2 2)
        (prog1 (list (length (overlays-at 1))
                     (length (overlays-at 2))
                     (length (overlays-at 3)))
          (kill-buffer buf))))
    '(0 0 0))

;; == overlays-in ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (make-overlay 2 6)
        (make-overlay 4 8)
        (prog1 (list (length (overlays-in 1 11))
                     (length (overlays-in 5 6)))
          (kill-buffer buf))))
    '(2 2))
;; boundaries: [2,4) vs (4,10] no overlap; (1,2] no; [2,4) yes
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (make-overlay 2 4)
        (prog1 (list (length (overlays-in 4 10))
                     (length (overlays-in 1 2))
                     (length (overlays-in 2 4))
                     (length (overlays-in 10 1)))
          (kill-buffer buf))))
    '(0 0 1 0))
;; empty overlay IS counted by overlays-in
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abc")
        (make-overlay 2 2)
        (prog1 (length (overlays-in 1 4))
          (kill-buffer buf))))
    1)

;; == next/previous-overlay-change ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (make-overlay 3 5)
        (make-overlay 7 9)
        (prog1 (list (next-overlay-change 1)
                     (next-overlay-change 3)
                     (next-overlay-change 5)
                     (next-overlay-change 9)
                     (next-overlay-change (point-max)))
          (kill-buffer buf))))
    '(3 5 7 11 11))
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (make-overlay 3 5)
        (make-overlay 7 9)
        (prog1 (list (previous-overlay-change 10)
                     (previous-overlay-change 7)
                     (previous-overlay-change 3)
                     (previous-overlay-change (point-min)))
          (kill-buffer buf))))
    '(9 5 1 1))
;; no overlays: boundaries
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abc")
        (prog1 (list (next-overlay-change 1)
                     (previous-overlay-change 3))
          (kill-buffer buf))))
    '(4 1))

;; == overlay-lists ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (let ((o (make-overlay 3 5))
              (ls (overlay-lists)))
          (prog1 (list (consp ls)
                       (not (null (memq o (car ls))))
                       (not (null (memq o (cdr ls)))))
            (kill-buffer buf)))))
    '(t t nil))

;; == delete-all-overlays ==
(qa (let ((buf (generate-new-buffer " *x*")))
      (with-current-buffer buf
        (insert "abcdefghij")
        (make-overlay 2 4)
        (make-overlay 6 8)
        (delete-all-overlays)
        (prog1 (length (overlays-in 1 11))
          (kill-buffer buf))))
    0)

;; == overlays are per-buffer ==
(qa (let ((b1 (generate-new-buffer " *x*"))
          (b2 (generate-new-buffer " *y*")))
      (unwind-protect
          (progn
            (with-current-buffer b1 (insert "0123456789") (make-overlay 2 4))
            (with-current-buffer b2
              (insert "0123456789")
              (length (overlays-in 1 10))))
        (kill-buffer b1) (kill-buffer b2)))
    0)

(princ (qa--done))
