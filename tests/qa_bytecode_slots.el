;;; -*- lexical-binding: t; -*-
;; Issue #255: function slots are readable but aset rejects the object.
(defun qa255-function (kind)
  (cond ((eq kind 'four) (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1))
        ((eq kind 'six) (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1 "qa255" '(interactive)))
        (t (read "#[0 \"\\300\\207\" [11 22] 1]"))))
(defun qa255-reject (kind index value)
  (let* ((f (qa255-function kind))
         (slots (mapcar (lambda (n) (aref f n)) (number-sequence 0 (1- (length f)))))
         (before (funcall f))
         (result (condition-case err (progn (aset f index value) '(stored)) (error err)))
         (after (mapcar (lambda (n) (aref f n)) (number-sequence 0 (1- (length f)))))
         (same (equal slots after)))
   (list (car result) (cadr result)
         (eq (nth 2 result) (if (eq (cadr result) 'arrayp) f index))
         same before (if same (funcall f) 'changed))))
(qa
(qa255-reject 'four -1 nil)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 0 1027)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 1 (unibyte-string 193 135))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 2 (vector 33))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 3 20)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 4 "replacement")
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 5 '(interactive))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four 6 nil)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'four nil nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'four 'bad nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'four 1.5 nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'four (expt 2 100) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'four (point-marker) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'four (make-marker) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'six -1 nil)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 0 1027)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 1 (unibyte-string 193 135))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 2 (vector 33))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 3 20)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 4 "replacement")
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 5 '(interactive))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six 6 nil)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'six nil nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'six 'bad nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'six 1.5 nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'six (expt 2 100) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'six (point-marker) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'six (make-marker) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'reader -1 nil)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 0 1027)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 1 (unibyte-string 193 135))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 2 (vector 33))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 3 20)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 4 "replacement")
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 5 '(interactive))
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader 6 nil)
'(wrong-type-argument arrayp t t 11 11)
)

(qa
(qa255-reject 'reader nil nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'reader 'bad nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'reader 1.5 nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'reader (expt 2 100) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'reader (point-marker) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(qa255-reject 'reader (make-marker) nil)
'(wrong-type-argument fixnump t t 11 11)
)

(qa
(let ((f (qa255-function 'four)))
  (list (funcall f)
	(condition-case err (aset f 2 [33])
	  (error (list (car err) (cadr err))))
	(funcall f) (progn (aset (aref f 2) 0 33) (funcall f))
	(progn (aset (aref f 1) 0 193) (funcall f))))
'(11 (wrong-type-argument arrayp) 11 33 22)
)

(qa
(let
    ((v (vector 1 2)) (s (copy-sequence "ab")) (r (record 'tag 1))
     (b (bool-vector nil nil)) (ct (make-char-table nil 0)))
  (list (aset v 1 3) (append v nil) (aset s 1 99) s (aset r 1 4)
	(aref r 1) (aset b 1 t) (aref b 1) (aset ct 97 5) (aref ct 97)))
'(3 (1 3) 99 "ac" 4 4 t t 5 5)
)


(princ (qa--done)) (terpri)
