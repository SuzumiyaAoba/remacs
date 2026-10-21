;;; -*- lexical-binding: t; -*-
;; Issue #254: in-place instruction-string mutations invalidate cached code.
(defvar qa254-code nil)
(defvar qa254-function nil)
(defvar qa254-entered nil)
(defvar qa254-watched nil)
(defvar qa254-unrelated nil)
(qa
(let*
    ((f (make-byte-code 0 (unibyte-string 192 135) [11 22] 1))
     (before (funcall f)))
  (aset (aref f 1) 0 193) (list before (funcall f)))
'(11 22)
)

(qa
(let*
    ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1))
     (before (funcall f)))
  (aset (aref f 2) 0 33) (list before (funcall f)))
'(11 33)
)

(qa
(let*
    ((code (unibyte-string 192 135))
     (constants (vconcat (number-sequence 0 63)))
     (f (make-byte-code 0 code constants 1)))
  (list (funcall f)
	(mapcar (lambda (n) (aset code 0 (+ 192 n)) (funcall f))
		(number-sequence 0 63))
	(progn (aset code 0 192) (funcall f))))
'(0
  (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25
     26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
     48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63)
  0)
)

(qa
(let*
    ((code (unibyte-string 129 0 0 135))
     (f (make-byte-code 0 code [11 22 33 44] 1)))
  (list (funcall f) (progn (aset code 1 1) (funcall f))
	(progn (aset code 1 3) (funcall f))
	(progn (aset code 1 0) (funcall f))))
'(11 22 44 11)
)

(qa
(let*
    ((code (unibyte-string 129 0 0 135))
     (constants (make-vector 257 nil)) (_ (aset constants 0 'zero))
     (_ (aset constants 256 'high))
     (f (make-byte-code 0 code constants 1)))
  (list (funcall f) (progn (aset code 2 1) (funcall f))
	(progn (aset code 2 0) (funcall f))))
'(zero high zero)
)

(qa
(let*
    ((code (unibyte-string 192 135))
     (f (make-byte-code 0 code [11 22] 1))
     (g (make-byte-code 0 code [33 44] 1)))
  (list (funcall f) (funcall g) (progn (aset code 0 193) (funcall f))
	(funcall g)))
'(11 33 22 44)
)

(qa
(let*
    ((code (unibyte-string 192 135)) (copy (copy-sequence code))
     (f (make-byte-code 0 code [11 22] 1))
     (g (make-byte-code 0 copy [33 44] 1)))
  (list (funcall f) (funcall g) (progn (aset code 0 193) (funcall f))
	(funcall g)))
'(11 33 22 33)
)

(qa
(let*
    ((code (unibyte-string 192 135))
     (f (make-byte-code 0 code [11 22] 1)) (g f))
  (list (eq code (aref f 1)) (eq code (aref g 1)) (funcall f)
	(funcall g) (progn (aset code 0 193) (funcall f)) (funcall g)))
'(t t 11 11 22 22)
)

(qa
(let*
    ((code (unibyte-string 192 135))
     (f (make-byte-code 0 code [11 22] 1)) (before (funcall f)))
  (aset code 0 193) (garbage-collect)
  (list before (funcall f) (eq code (aref f 1))
	(progn (put-text-property 0 1 'face 'bold code) (funcall f))))
'(11 22 t 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 193 135))
     (f
      (make-byte-code 0 qa254-code
		      (vector (lambda nil (aset qa254-code 3 194)) 11
			      22)
		      1)))
  (list (funcall f) (funcall f)))
'(22 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 193 135))
     (qa254-entered nil)
     (qa254-function
      (make-byte-code 0 qa254-code
		      (vector
		       (lambda nil
			 (unless qa254-entered
			   (setq qa254-entered t)
			   (aset qa254-code 3 194)
			   (funcall qa254-function)))
		       11 22)
		      1)))
  (list (funcall qa254-function) (funcall qa254-function)))
'(22 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 129 1 0 135))
     (f
      (make-byte-code 0 qa254-code
		      (vector
		       (lambda nil
			 (store-substring qa254-code 4
					  (unibyte-string 2)))
		       11 22)
		      1)))
  (list (funcall f) (funcall f)))
'(22 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 129 1 0 135))
     (f
      (make-byte-code 0 qa254-code
		      (vector
		       (lambda nil
			 (subst-char-in-string 1 2 qa254-code t))
		       11 22)
		      1)))
  (list (funcall f) (funcall f)))
'(22 22)
)

(qa
(let* ((f #[0 "\300\207" [11 22] 1]) (before (funcall f)))
  (aset (aref f 1) 0 193) (list before (funcall f)))
'(11 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 193 135))
     (constants (vector (lambda nil (aset qa254-code 3 194)) 11 22)))
  (list (byte-code qa254-code constants 1)
	(byte-code qa254-code constants 1)))
'(22 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 129 1 0 135))
     (constants
      (vector
       (lambda nil (store-substring qa254-code 4 (unibyte-string 2)))
       11 22)))
  (list (byte-code qa254-code constants 1)
	(byte-code qa254-code constants 1)))
'(22 22)
)

(qa
(let*
    ((qa254-code (unibyte-string 193 16 193 135)) (qa254-watched nil)
     (watcher
      (lambda (_symbol _new _operation _buffer)
	(aset qa254-code 2 194)))
     (f (make-byte-code 0 qa254-code [qa254-watched 11 22] 1)))
  (unwind-protect
      (progn
	(add-variable-watcher 'qa254-watched watcher)
	(list (funcall f) (funcall f) qa254-watched))
    (remove-variable-watcher 'qa254-watched watcher)))
'(22 22 11)
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 193 135))
     (qa254-unrelated (copy-sequence "a"))
     (f
      (make-byte-code 0 qa254-code
		      (vector (lambda nil (aset qa254-unrelated 0 98))
			      11 22)
		      1)))
  (list (funcall f) (funcall f) qa254-unrelated))
'(11 11 "b")
)

(qa
(let*
    ((qa254-code (unibyte-string 192 32 136 193 135))
     (f
      (make-byte-code 0 qa254-code
		      (vector
		       (lambda nil (garbage-collect)
			 (aset qa254-code 3 194) (garbage-collect))
		       11 22)
		      1)))
  (list (funcall f) (funcall f)))
'(22 22)
)


(princ (qa--done)) (terpri)
