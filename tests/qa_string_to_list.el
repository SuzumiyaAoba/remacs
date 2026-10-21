;;; -*- lexical-binding: t; -*-
;; Issue #256: string-to-list is append with a nil tail.
(defun qa256-fresh (source)
  (let ((out (string-to-list source)))
    (list out (eq source out)
          (if (consp source) (eq (cdr source) (cdr out)) nil))))
(defun qa256-error (source)
  (condition-case err (string-to-list source)
    (error (list (car err) (cadr err) (eq (nth 2 err) source)))))
(defun qa256-cycle (size entry)
  (let ((source (number-sequence 0 (1- size))))
    (setcdr (last source) (nthcdr entry source))
    (condition-case err (progn (string-to-list source) 'accepted)
      (error
       (let ((node source) (position 0) (found nil))
        (while (and (< position size) (not found))
         (if (eq node (cadr err)) (setq found position)
          (setq node (cdr node) position (1+ position))))
        (list (car err) found))))))
(qa
(qa256-fresh nil)
'(nil t nil)
)

(qa
(qa256-fresh (intern "nil"))
'(nil t nil)
)

(qa
(qa256-fresh (list 1))
'((1) nil t)
)

(qa
(qa256-fresh (list 1 2))
'((1 2) nil nil)
)

(qa
(qa256-fresh (list 1 2 3 4))
'((1 2 3 4) nil nil)
)

(qa
(qa256-fresh (list 'a nil 'b))
'((a nil b) nil nil)
)

(qa
(qa256-fresh [])
'(nil nil nil)
)

(qa
(qa256-fresh [1 2 nil "x"])
'((1 2 nil "x") nil nil)
)

(qa
(qa256-fresh (bool-vector))
'(nil nil nil)
)

(qa
(qa256-fresh (bool-vector t nil t nil))
'((t nil t nil) nil nil)
)

(qa
(qa256-fresh "")
'(nil nil nil)
)

(qa
(qa256-fresh "abc")
'((97 98 99) nil nil)
)

(qa
(qa256-fresh (string-make-multibyte "abc"))
'((97 98 99) nil nil)
)

(qa
(qa256-fresh (unibyte-string 0 127 128 255))
'((0 127 128 255) nil nil)
)

(qa
(qa256-fresh "日é")
'((26085 233) nil nil)
)

(qa
(qa256-fresh (string 55296 1114112 4194176))
'((55296 1114112 4194176) nil nil)
)

(qa
(qa256-fresh (propertize "ab" 'face 'bold))
'((97 98) nil nil)
)

(qa
(let* ((source (list 1 2 3)) (out (string-to-list source)))
  (setcar out 9) (setcar (cdr out) 8) (setcdr (cdr out) nil)
  (list source out))
'((1 2 3) (9 8))
)

(qa
(let*
    ((cell (list 1)) (vec (vector 2)) (text (copy-sequence "a"))
     (source (list cell vec text)) (out (string-to-list source)))
  (list (eq source out) (eq (cdr source) (cdr out))
	(eq (nth 0 source) (nth 0 out))
	(eq (nth 1 source) (nth 1 out))
	(eq (nth 2 source) (nth 2 out))
	(progn (setcar (car out) 9) (car source))))
'(nil nil t t t (9))
)

(qa
(condition-case err (string-to-list '(1 . 2)) (error err))
'(wrong-type-argument listp 2)
)

(qa
(condition-case err (string-to-list '(1 2 . tail)) (error err))
'(wrong-type-argument listp tail)
)

(qa
(condition-case err (string-to-list (cons 1 "bad")) (error err))
'(wrong-type-argument listp "bad")
)

(qa
(condition-case err (string-to-list (cons 1 [2 3])) (error err))
'(wrong-type-argument listp [2 3])
)

(qa
(qa256-error 42)
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error 1.5)
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error (expt 2 100))
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error 'bad)
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error (point-marker))
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error (record 'tag 1))
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error (make-char-table nil))
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error (symbol-function 'car))
'(wrong-type-argument sequencep t)
)

(qa
(qa256-error (make-byte-code 0 (unibyte-string 192 135) [11] 1))
'(0 "\300\207" [11] 1)
)

(qa
(qa256-cycle 1 0)
'(circular-list 0)
)

(qa
(qa256-cycle 2 0)
'(circular-list 1)
)

(qa
(qa256-cycle 2 1)
'(circular-list 1)
)

(qa
(qa256-cycle 3 0)
'(circular-list 0)
)

(qa
(qa256-cycle 3 1)
'(circular-list 1)
)

(qa
(qa256-cycle 3 2)
'(circular-list 2)
)

(qa
(qa256-cycle 4 0)
'(circular-list 3)
)

(qa
(qa256-cycle 4 1)
'(circular-list 3)
)

(qa
(qa256-cycle 4 2)
'(circular-list 3)
)

(qa
(qa256-cycle 4 3)
'(circular-list 3)
)

(qa
(qa256-cycle 5 0)
'(circular-list 2)
)

(qa
(qa256-cycle 5 1)
'(circular-list 3)
)

(qa
(qa256-cycle 5 2)
'(circular-list 3)
)

(qa
(qa256-cycle 5 3)
'(circular-list 3)
)

(qa
(qa256-cycle 5 4)
'(circular-list 4)
)

(qa
(qa256-error (eval '(lambda nil 11) t))
'(nil (11) (t))
)

(qa
(qa256-error (eval '(lambda (x) (+ x 1)) t))
'((x) ((+ x 1)) (t))
)

(qa
(qa256-error (eval '(lambda nil "doc" (interactive) 11) t))
'(nil (11) (t) nil "doc" nil)
)

(qa
(qa256-error
 (make-byte-code 0 (unibyte-string 192 135) [11] 1 "doc"
		 '(interactive)))
'(0 "\300\207" [11] 1 "doc" (interactive))
)

(qa
(qa256-error (eval '(lambda nil 11) nil))
'(nil (11) nil)
)

(qa
(qa256-error (eval '(lambda nil "doc" (interactive) 11) nil))
'(nil (11) nil nil "doc" nil)
)

(qa
(append)
'nil
)

(qa
(let ((tail (list 'a 'b)))
  (list (eq (append tail) tail) (eq (append nil nil tail) tail)))
'(t t)
)

(qa
(let*
    ((source (list 1 2)) (tail (list 3 4))
     (out (append source [9] "A" (bool-vector t nil) tail)))
  (list out (eq (nthcdr 6 out) tail) (eq source out)
	(progn (setcar out 8) source)))
'((8 2 9 65 t nil 3 4) t nil (1 2))
)

(qa
(append '(1 2) 'tail)
'(1 2 . tail)
)

(qa
(let ((tail (vector 1 2))) (eq (append nil tail) tail))
't
)

(qa
(condition-case err (append '(1) (cons 2 'bad) nil) (error err))
'(wrong-type-argument listp bad)
)

(qa
(append (make-byte-code 0 (unibyte-string 192 135) [11] 1) [end]
	'(tail))
'(0 "\300\207" [11] 1 end tail)
)


(princ (qa--done)) (terpri)
