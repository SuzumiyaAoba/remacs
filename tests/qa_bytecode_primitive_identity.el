;;; -*- lexical-binding: nil; -*-
;; Dedicated opcodes keep primitive identity across unintern/reintern (#596).
(let ((profiles
       (list
        (list "+" (byte-compile '(lambda (x y) (+ x y))) '(1 2) 3 92)
        (list "-" (byte-compile '(lambda (x y) (- x y))) '(1 2) -1 90)
        (list "*" (byte-compile '(lambda (x y) (* x y))) '(2 3) 6 95)
        (list "1+" (byte-compile '(lambda (x) (1+ x))) '(2) 3 84)
        (list "1-" (byte-compile '(lambda (x) (1- x))) '(2) 1 83)
        (list "length" (byte-compile '(lambda (x) (length x))) '((a b)) 2 71)
        (list "car" (byte-compile '(lambda (x) (car x))) '((a b)) 'a 64)
        (list "aset" (byte-compile '(lambda (x) (aset x 0 65)))
              (list (copy-sequence "ab")) 65 73)
        (list "string=" (byte-compile '(lambda (x y) (string= x y))) '("a" "a") t 152))))
  (dolist (profile profiles)
    (let* ((name (nth 0 profile)) (function (nth 1 profile))
           (arguments (nth 2 profile)) (expected (nth 3 profile))
           (opcode (nth 4 profile))
           (original (symbol-function (intern name))))
      (qa (not (null (memq opcode (string-to-list (aref function 1))))) t)
      (unintern name nil)
      (qa (apply function arguments) expected)
      ;; Calling an opcode does not silently put the name back in obarray.
      (qa (intern-soft name) nil)
      (let ((replacement (intern name)))
        (unwind-protect
            (progn
              (fset replacement (lambda (&rest _) 'shadow))
              (qa (apply function arguments) expected)
              (qa (apply replacement arguments) 'shadow))
          (fset replacement original))))))
(princ (qa--done))
