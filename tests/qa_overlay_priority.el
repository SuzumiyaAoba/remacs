;;; qa_overlay_priority.el --- Overlay priority and containment -*- lexical-binding: t; -*-
;; Compare sorted queries, effective properties and the selected overlay.

(defun qa309-order (abegin aend bbegin bend apriority bpriority &optional category hide)
  (with-temp-buffer
    (insert "abcdefghijkl")
    (let ((a (make-overlay abegin aend)) (b (make-overlay bbegin bend)))
      (if category
          (progn
            (put 'qa309-category 'priority apriority)
            (overlay-put a 'category 'qa309-category)
            (when (eq category 'nil-shadow) (overlay-put a 'priority nil)))
        (overlay-put a 'priority apriority))
      (overlay-put b 'priority bpriority)
      (unless hide (overlay-put a 'face 'bold))
      (unless (eq hide 'all) (overlay-put b 'face 'italic))
      (when (eq hide 'all) (put-text-property 1 (point-max) 'face 'underline))
      (let ((hits (overlays-at 4 t))
            (property (get-char-property-and-overlay 4 'face)))
        (list (eq a (car hits)) (get-char-property 4 'face)
              (eq a (cdr property)) (car property)
              (mapcar (lambda (overlay) (if (eq overlay a) 'a 'b)) hits))))))

(defun qa309-crossing-set ()
  (with-temp-buffer
    (insert "abcdefghijkl")
    (dotimes (n 64)
      (let* ((spec (nth (% n 3) '((1 10 10) (2 8 0) (3 12 5))))
             (overlay (make-overlay (nth 0 spec) (nth 1 spec))))
        (overlay-put overlay 'priority (cons nil (nth 2 spec)))
        (overlay-put overlay 'qa309-index n)))
    (let ((hits (overlays-at 4 t)) (value (get-char-property 4 'qa309-index)))
      (list (= (length hits) 64)
            (equal (sort (mapcar (lambda (o) (overlay-get o 'qa309-index)) hits) #'<)
                   (number-sequence 0 63))
            (and (integerp value) (<= 0 value) (< value 64))))))

;; secondary priority
(qa (qa309-order 1 7 1 7 '(nil . 10) '(nil . 1))
    '(t bold t bold (a b)))

;; reversed secondary
(qa (qa309-order 1 7 1 7 '(nil . 1) '(nil . 10))
    '(nil italic nil italic (b a)))

;; higher primary beats nesting
(qa (qa309-order 1 8 3 6 2 1)
    '(t bold t bold (a b)))

;; lower primary loses nesting
(qa (qa309-order 3 6 1 8 1 2)
    '(nil italic nil italic (b a)))

;; cons primary matches integer
(qa (qa309-order 3 5 1 5 '(5 . 0) 5)
    '(t bold t bold (a b)))

;; shared beginning
(qa (qa309-order 1 5 1 7 '(nil . 1) '(nil . 10))
    '(t bold t bold (a b)))

;; shared end
(qa (qa309-order 3 5 1 5 '(nil . 1) '(nil . 10))
    '(t bold t bold (a b)))

;; nested secondary ignored
(qa (qa309-order 3 6 1 8 '(nil . -10) '(nil . 100))
    '(t bold t bold (a b)))

;; reverse nesting
(qa (qa309-order 1 8 3 6 '(nil . 100) '(nil . -10))
    '(nil italic nil italic (b a)))

;; crossing higher secondary
(qa (qa309-order 1 6 3 8 '(nil . 10) '(nil . 1))
    '(t bold t bold (a b)))

;; crossing lower secondary
(qa (qa309-order 1 6 3 8 '(nil . 1) '(nil . 10))
    '(nil italic nil italic (b a)))

;; reverse crossing higher
(qa (qa309-order 3 8 1 6 '(nil . 10) '(nil . 1))
    '(t bold t bold (a b)))

;; reverse crossing lower
(qa (qa309-order 3 8 1 6 '(nil . 1) '(nil . 10))
    '(nil italic nil italic (b a)))

;; negative primary
(qa (qa309-order 3 5 1 7 -1 0)
    '(nil italic nil italic (b a)))

;; non-fixnum cons primary
(qa (qa309-order 1 7 1 7 '(1.5 . 10) '(0 . 1))
    '(t bold t bold (a b)))

;; non-fixnum secondary
(qa (qa309-order 1 7 1 7 '(1 . 2.5) '(1 . 1))
    '(nil italic nil italic (b a)))

;; bignum cons primary
(qa (qa309-order 1 7 1 7 '(#x100000000000000000 . 10) '(nil . 1))
    '(t bold t bold (a b)))

;; bignum secondary
(qa (qa309-order 1 7 1 7 '(nil . #x100000000000000000) '(nil . 1))
    '(nil italic nil italic (b a)))

;; proper list priority
(qa (qa309-order 3 5 1 7 '(5 99) 5)
    '(t bold t bold (a b)))

;; category priority
(qa (qa309-order 1 7 1 7 '(nil . 10) '(nil . 1) t)
    '(t bold t bold (a b)))

;; nil shadows category
(qa (qa309-order 1 7 1 7 '(nil . 10) '(nil . 1) 'nil-shadow)
    '(nil italic nil italic (b a)))

;; missing face falls through
(qa (qa309-order 1 7 1 7 2 1 nil t)
    '(t italic nil italic (a b)))

;; text property fallback
(qa (qa309-order 1 7 1 7 2 1 nil 'all)
    '(t underline nil underline (a b)))

;; crossing set remains a permutation
(qa (qa309-crossing-set)
    '(t t t))

(princ (qa--done))
