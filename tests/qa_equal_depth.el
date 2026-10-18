;;; -*- lexical-binding: nil; -*-
;; Issue #182: GNU's recursion limit is an error, not an equality result.
(defun qa182-nest (kind depth leaf)
  (dotimes (_ depth leaf)
    (setq leaf (cond ((eq kind 'list) (list leaf))
                     ((eq kind 'vector) (vector leaf))
                     (t (record 'qa182 leaf))))))
(defun qa182-result (a b)
  (condition-case err (equal a b) (error err)))

;; The error check precedes even an identical atomic leaf at depth 201.
;; Different types at that depth must signal too. At depth 200 all three
;; comparisons still reach their leaf normally.
(dolist (kind '(list vector record))
  (dolist (depth '(10 11 199 200 201 205))
    (let ((a (qa182-nest kind depth 1))
          (b (qa182-nest kind depth 2))
          (c (qa182-nest kind depth 1))
          (d (qa182-nest kind depth "1")))
      (qa (qa182-result a b)
          (if (> depth 200) '(error "Stack overflow in equal") nil))
      (qa (qa182-result a c)
          (if (> depth 200) '(error "Stack overflow in equal") t))
      (qa (qa182-result a d)
          (if (> depth 200) '(error "Stack overflow in equal") nil)))))

;; The top-level identity shortcut and an early mismatch never descend.
(qa (let ((a (qa182-nest 'list 300 1))) (equal a a)) t)
(qa (equal (vector 1 (qa182-nest 'vector 300 1))
           (vector 2 (qa182-nest 'vector 300 2))) nil)
(qa (equal (qa182-nest 'list 200 [1])
           (qa182-nest 'list 200 [])) nil)

;; A cons at depth 200 must compare its car at depth 201. Conversely,
;; after a car at depth 200, shared cdrs remain an identity shortcut.
(qa-err-data (equal (qa182-nest 'list 200 (list 1))
                    (qa182-nest 'list 200 (list 2)))
  'error '("Stack overflow in equal"))
(qa (equal (qa182-nest 'list 199 (cons 1 'same))
           (qa182-nest 'list 199 (cons 1 'same))) t)
(qa (equal (qa182-nest 'list 199 (cons 1 'left))
           (qa182-nest 'list 199 (cons 1 'right))) nil)

;; List tails are iterative; their length is not recursive depth.
(qa (equal (make-list 10000 1) (make-list 10000 1)) t)
(qa (equal (make-list 10000 1) (append (make-list 9999 1) '(2))) nil)

;; Repeated recursive object pairs are recorded after depth ten.
;; A period 189 repeats a recorded pair at depth 200; period 190 reaches
;; the error guard first, before that repeated-pair shortcut.
(defun qa182-car-ring (size)
  (let* ((root (cons nil nil)) (tail root))
    (dotimes (_ (1- size))
      (let ((next (cons nil nil)))
        (setcar tail next)
        (setq tail next)))
    (setcar tail root)
    root))
(qa (qa182-result (qa182-car-ring 189) (qa182-car-ring 189)) t)
(qa (qa182-result (qa182-car-ring 190) (qa182-car-ring 190))
    '(error "Stack overflow in equal"))

;; All public structural-comparison callers must propagate the same error.
(let ((a (qa182-nest 'list 205 1)) (b (qa182-nest 'list 205 2)))
  (qa-err-data (equal a b) 'error '("Stack overflow in equal"))
  (qa-err-data (equal-including-properties a b) 'error '("Stack overflow in equal"))
  (qa-err-data (member a (list b)) 'error '("Stack overflow in equal"))
  (qa-err-data (assoc a (list (cons b 'value))) 'error '("Stack overflow in equal"))
  (qa-err-data (rassoc a (list (cons 'key b))) 'error '("Stack overflow in equal"))
  (qa-err-data (assoc-default a (list (cons b 'value))) 'error '("Stack overflow in equal"))
  (qa-err-data (delete a (list b)) 'error '("Stack overflow in equal"))
  (qa-err-data (delete a (vector b)) 'error '("Stack overflow in equal"))
  (qa-err-data (remove a (list b)) 'error '("Stack overflow in equal"))
  (qa-err-data (remove a (vector b)) 'error '("Stack overflow in equal"))
  (qa-err-data (lax-plist-get (list b 'value) a) 'error '("Stack overflow in equal"))
  (qa-err-data (lax-plist-put (list b 'value) a 'new) 'error '("Stack overflow in equal"))
  ;; byte-constant, byte-constant+1, byte-equal, byte-return.
  (qa-err-data (byte-code (unibyte-string 192 193 154 135) (vector a b) 2)
    'error '("Stack overflow in equal"))
  ;; Identity-based callers must remain unaffected by structural depth.
  (qa (memq a (list b)) nil)
  (qa (memql a (list b)) nil)
  (qa (assq a (list (cons b 'value))) nil)
  (qa (eq (car (member a (list a b))) a) t))
(princ (qa--done)) (terpri)
