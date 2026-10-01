;; Marker primitives — GNU 31.1 -Q --batch ground truth.
;; Pinned semantics:
;;  * A fresh `make-marker' is detached: marker-buffer/marker-position nil.
;;  * `set-marker' returns its marker arg (eq), accepts integer/marker
;;    POSITION, clamps out-of-range positions to [point-min size+1] of the
;;    REAL buffer (ignores narrowing), and a nil POSITION detaches.
;;  * `point-marker' returns a NEW marker each call (not eq to itself).
;;  * `copy-marker' accepts a marker or an integer position (or nil =>
;;    detached); TYPE arg sets the insertion type.
;;  * marker-insertion-type nil: text inserted AT the marker goes AFTER it
;;    (marker does not advance); t: marker advances past inserted text.
;;    Text inserted BEFORE the position always advances the marker.
;;  * `insert-before-markers' advances even insertion-type-nil markers.
;;  * `set-marker-insertion-type' returns the TYPE argument (not marker).
;;  * Killing a buffer detaches its markers; markers stay `markerp'.

;; == predicates / detached markers ==
(qa (markerp (make-marker)) t)
(qa (markerp 5) nil)
(qa (markerp "x") nil)
(qa (markerp '(1 2)) nil)
(qa (markerp (point-marker)) t)
(qa (marker-position (make-marker)) nil)
(qa (marker-buffer (make-marker)) nil)
(qa (marker-insertion-type (make-marker)) nil)

;; == set-marker basics ==
(qa (let ((m (make-marker))) (eq (set-marker m 1) m)) t)
(qa (let ((m (make-marker)))
      (set-marker m 1)
      (list (marker-position m) (eq (marker-buffer m) (current-buffer))))
    '(1 t))
;; position clamping to [1, size+1]
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (make-marker)))
        (set-marker m -5) (marker-position m)))
    1)
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (make-marker)))
        (set-marker m 0) (marker-position m)))
    1)
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (make-marker)))
        (set-marker m 99) (marker-position m)))
    7)
;; explicit BUFFER argument
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (make-marker)) (buf (current-buffer)))
        (set-marker m 3 buf)
        (list (marker-position m) (eq (marker-buffer m) buf))))
    '(3 t))
;; POSITION may itself be a marker: buffer+pos are copied
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((a (set-marker (make-marker) 4))
            (b (make-marker)) (buf (current-buffer)))
        (set-marker b a)
        (list (marker-position b) (eq (marker-buffer b) buf))))
    '(4 t))
;; nil POSITION detaches
(qa (with-temp-buffer
      (insert "ab")
      (let ((m (set-marker (make-marker) 2)))
        (set-marker m nil)
        (list (marker-position m) (marker-buffer m))))
    '(nil nil))

;; == point-marker ==
(qa (with-temp-buffer
      (insert "abcdef")
      (goto-char 3)
      (list (marker-position (point-marker))
            (eq (marker-buffer (point-marker)) (current-buffer))))
    '(3 t))
(qa (with-temp-buffer (eq (point-marker) (point-marker))) nil)

;; == copy-marker ==
(qa (with-temp-buffer
      (insert "abcdef")
      (let* ((m (set-marker (make-marker) 3))
             (c (copy-marker m)))
        (list (marker-position c)
              (eq (marker-buffer c) (current-buffer))
              (markerp c)
              (marker-insertion-type c))))
    '(3 t t nil))
;; integer source => new marker in current buffer at that pos
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((c (copy-marker 5)))
        (list (marker-position c)
              (eq (marker-buffer c) (current-buffer)))))
    '(5 t))
;; second arg sets insertion type
(qa (with-temp-buffer (marker-insertion-type (copy-marker 2 t))) t)
;; copying a detached marker stays detached
(qa (let ((c (copy-marker (make-marker))))
      (list (marker-position c) (marker-buffer c)))
    '(nil nil))
;; bare (copy-marker) with nil arg => detached marker
(qa (let ((c (copy-marker)))
      (list (markerp c) (marker-position c) (marker-buffer c)))
    '(t nil nil))

;; == insertion type ==
;; type nil: insertion AT the marker does not advance it
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (copy-marker 3)))
        (goto-char 3) (insert "X") (marker-position m)))
    3)
;; type t: advances past inserted text
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (copy-marker 3 t)))
        (goto-char 3) (insert "X") (marker-position m)))
    4)
;; insertion BEFORE the marker always advances it
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (copy-marker 3)))
        (goto-char 1) (insert "Y") (marker-position m)))
    4)
;; insert-before-markers advances even type-nil markers
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (copy-marker 3)))
        (goto-char 3) (insert-before-markers "Z")
        (marker-position m)))
    4)
;; deleting text before the marker pulls it back
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (copy-marker 4)))
        (delete-region 1 3) (marker-position m)))
    2)
;; marker at deleted-region start stays at the deletion start
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((m (copy-marker 2)))
        (delete-region 2 4) (marker-position m)))
    2)

;; == set-marker-insertion-type ==
(qa (with-temp-buffer
      (insert "ab")
      (let ((m (copy-marker 2)))
        (set-marker-insertion-type m t)
        (marker-insertion-type m)))
    t)
(qa (with-temp-buffer
      (insert "ab")
      (let ((m (copy-marker 2)))
        (set-marker-insertion-type m t)
        (set-marker-insertion-type m nil)
        (marker-insertion-type m)))
    nil)
;; return value is the TYPE argument, not the marker
(qa (with-temp-buffer
      (insert "ab")
      (let ((m (copy-marker 2)))
        (list (set-marker-insertion-type m t)
              (markerp m) (marker-insertion-type m))))
    '(t t t))
(qa (with-temp-buffer
      (insert "ab")
      (set-marker-insertion-type (copy-marker 1) nil))
    nil)

;; == narrowing: set-marker ignores the restriction ==
(qa (with-temp-buffer
      (insert "abcdef")
      (narrow-to-region 2 5)
      (prog1 (let ((m (make-marker)))
               (set-marker m 6) (marker-position m))
        (widen)))
    6)
(qa (with-temp-buffer
      (insert "abcdef")
      (narrow-to-region 2 5)
      (prog1 (let ((m (make-marker)))
               (set-marker m 1) (marker-position m))
        (widen)))
    1)

;; == killed buffers detach markers ==
(qa (let* ((buf (generate-new-buffer " *x*"))
           (m (progn (with-current-buffer buf (insert "abc"))
                     (set-marker (make-marker) 2 buf))))
      (kill-buffer buf)
      (list (markerp m) (marker-position m) (marker-buffer m)))
    '(t nil nil))

;; == marker equality: equal compares buffer+position ==
(qa (with-temp-buffer
      (insert "abc")
      (let ((a (copy-marker 2)) (b (copy-marker 2)))
        (list (eq a b) (equal a b))))
    '(nil t))
(qa (with-temp-buffer
      (insert "abc")
      (let ((a (copy-marker 2))) (eq a a)))
    t)
;; different positions => not equal
(qa (with-temp-buffer
      (insert "abc")
      (equal (copy-marker 2) (copy-marker 3)))
    nil)

;; == errors ==
(qa-err (markerp) 'wrong-number-of-arguments)
(qa-err (marker-position) 'wrong-number-of-arguments)
(qa-err (marker-position 5) 'wrong-type-argument)
(qa-err (marker-buffer 'x) 'wrong-type-argument)
(qa-err (marker-insertion-type "a") 'wrong-type-argument)
;; remacs: "No buffer named" error instead of wrong-type-argument
(qa-err (set-marker (make-marker) 1 "not-a-buffer") 'wrong-type-argument)
(qa-err (copy-marker 'sym) 'wrong-type-argument)
(qa-err (copy-marker 1 t 'extra) 'wrong-number-of-arguments)
(qa-err (set-marker-insertion-type (make-marker)) 'wrong-number-of-arguments)

(princ (qa--done))
