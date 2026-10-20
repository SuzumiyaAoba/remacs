;; Evaluation & special forms — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - (eval "(+ 1 2)") returns the STRING, not 3: strings self-evaluate.
;;   - (eval FORM ENV): '((a . 5)) env -> (eval '(+ a 1) '((a . 5))) => 6.
;;   - apply's last arg must be a list: vector/int -> wrong-type-argument.
;;   - throw with no matching catch -> no-catch; catch nil does NOT
;;     catch throws of non-nil tags (still no-catch).
;;   - signal of a symbol with no error-property signals 'error.
;;   - (condition-case nil BODY (:success X)) -> X on success.
;;   - (if nil 1 2 3 4) evaluates ELSE forms; returns last => 4.
;;   - funcall of a mis-arity subr -> wrong-number-of-arguments.
;;   - eval runs in dynamic scope: (funcall (let ((b 1)) (lambda () b)))
;;     => void-variable — eval'd lambdas do not capture lets.

;; == funcall ==
(qa (funcall #'+ 1 2 3) 6)
(qa (funcall 'car '(a b)) 'a)
(qa (funcall #'1+ 5) 6)
(qa (funcall #'identity '(1 2)) '(1 2))
(qa (funcall (lambda (x) (* x x)) 5) 25)
(qa (funcall #'list) nil)
(qa-err (funcall #'1+ 1 2) 'wrong-number-of-arguments)
(qa-err (funcall 1 2) 'invalid-function)
(qa-err (funcall #'qa-no-such-fn 1) 'void-function)

;; == apply ==
(qa (apply #'+ '(1 2 3)) 6)
(qa (apply #'+ nil) 0)
(qa (apply #'list 1 2 '(3 4)) '(1 2 3 4))
(qa (apply #'list 1 nil) '(1))
(qa (apply #'concat '("ab" "cd")) "abcd")
(qa-err (apply #'+ [1 2]) 'wrong-type-argument)
(qa-err (apply #'+ 1 2 3) 'wrong-type-argument)

;; == closures & funcall through let ==
(qa (let ((n 10))
      (let ((get (lambda () n)) (inc (lambda () (setq n (1+ n)))))
        (funcall inc)
        (funcall get)))
    11)
(qa-err (funcall (let ((base 100)) (lambda (x) (+ base x))) 5)
        'void-variable)

;; == eval ==
(qa (eval '(+ 1 2)) 3)
(qa (eval '(let ((x 5)) (* x x))) 25)
(qa (eval "(+ 1 2)") "(+ 1 2)")
(qa (eval ''x) 'x)
(qa (eval '(+ a 1) '((a . 5))) 6)
(qa (eval '(when t 42)) 42)
(qa-err (eval 'qa-eval-undef-var) 'void-variable)

;; == function classification ==
(qa (functionp 'car) t)
(qa (functionp nil) nil)
(qa (functionp 'when) nil)
(qa (functionp (lambda (x) x)) t)
(qa (special-form-p 'if) t)
(qa (special-form-p 'car) nil)
(qa (special-form-p 'when) nil)
(qa (macrop 'when) t)
(qa (macrop 'if) nil)
(qa (subrp (symbol-function 'car)) t)
(qa (subrp (symbol-function 'when)) nil)

;; == cond ==
(qa (cond (nil 'a) (t 'b)) 'b)
(qa (cond) nil)
(qa (cond ('x)) 'x)
(qa (cond (t 'a) (t 'b)) 'a)
(qa (let ((n 2)) (cond ((= n 1) 'one) ((= n 2) 'two) (t 'other))) 'two)

;; == and / or / if / when / unless ==
(qa (and) t)
(qa (and 1 2 3) 3)
(qa (and 1 nil 3) nil)
(qa (or nil nil) nil)
(qa (or nil 'x 'y) 'x)
(qa (or 0 (error "unreached")) 0)
(qa (if t 'yes 'no) 'yes)
(qa (if nil 'yes) nil)
(qa (if nil 1 2 3 4) 4)
(qa (when t 'a 'b) 'b)
(qa (when nil 'a) nil)
(qa (unless nil 'a 'b) 'b)
(qa (unless t 'a) nil)

;; == prog1 / prog2 / progn ==
(qa (prog1 1 2 3) 1)
(qa (prog1 5) 5)
(qa (prog2 'a 'b 'c) 'b)
(qa (progn 1 2 3) 3)
(qa (progn) nil)
(qa (let ((x nil)) (prog1 'ret (push 1 x) (push 2 x)) x) '(2 1))

;; == let / let* / setq ==
(qa (let ((a 1) (b 2)) (+ a b)) 3)
(qa (let ((a 1)) (let ((a 2)) a)) 2)
(qa (let* ((a 1) (b (+ a 1))) (list a b)) '(1 2))
(qa (let* (a (b 2)) (list a b)) '(nil 2))
(qa (let ((a 1) (b 2)) (setq a 10 b 20) (+ a b)) 30)
(qa (let ((acc nil))
      (dolist (x '(1 2 3)) (push x acc))
      acc)
    '(3 2 1))
(qa (dolist (x '(1 2) 'done)) 'done)
(qa (let ((i 0)) (while (< i 3) (setq i (1+ i))) i) 3)
(qa (while nil 'never) nil)

;; == catch / throw ==
(qa (catch 'tag (throw 'tag 'v)) 'v)
(qa (catch 'tag 1 2 3) 3)
(qa (catch 'a (catch 'b (throw 'a 1) 2) 3) 1)
(qa (catch 'tag (condition-case nil (throw 'tag 42) (error 'bad))) 42)
(qa-err (throw 'nowhere 1) 'no-catch)
(qa-err (catch nil (throw 'tag 'v)) 'no-catch)

;; == unwind-protect ==
(qa (unwind-protect 'x 'y) 'x)
(qa (let ((log nil))
      (catch 'done
        (unwind-protect (throw 'done 'v) (push 'cleaned log)))
      log)
    '(cleaned))
(qa (let ((log nil))
      (condition-case nil
          (unwind-protect (error "boom") (push 'cleaned log))
        (error 'caught))
      log)
    '(cleaned))
(qa (let ((log nil))
      (condition-case nil
          (unwind-protect 'ok (push 'cleaned log))
        (error 'caught))
      (list 'ret log))
    '(ret (cleaned)))

;; == condition-case ==
(qa (condition-case nil (+ 1 2) (error 'bad)) 3)
(qa (condition-case nil (car 1) (error 'caught)) 'caught)
(qa (condition-case e (car 1) (error (car e))) 'wrong-type-argument)
(qa (condition-case e (error "oops") (error (cadr e))) "oops")
(qa (condition-case nil 5 (:success 'ok) (error 'bad)) 'ok)
(qa (condition-case nil (error "x") ((debug error) 'd) (error 'e)) 'd)
(qa (condition-case e (signal 'arith-error nil) (arith-error (car e)))
    'arith-error)
(qa (condition-case nil (signal 'arith-error nil) (error 'parent))
    'parent)
;; signal of a symbol with no error property signals 'error
(qa-err (signal 'qa-undef-error '(x)) 'error)
(qa (condition-case e (signal 'qa-undef-error '(x))
      (qa-undef-error 'child) (error (car e)))
    'error)

;; == save-excursion / save-current-buffer / save-restriction ==
(qa (let ((buf (generate-new-buffer " *qa-eval*")))
      (unwind-protect
          (with-current-buffer buf
            (insert "abcdef")
            (save-excursion (goto-char 1) (insert "ZZ"))
            (list (point) (buffer-string)))
        (kill-buffer buf)))
    '(9 "ZZabcdef"))
(qa (let ((buf (generate-new-buffer " *qa-eval*")))
      (unwind-protect
          (progn (save-current-buffer (set-buffer buf) 'inside)
                 (eq (current-buffer) buf))
        (kill-buffer buf)))
    nil)
(qa (let ((buf (generate-new-buffer " *qa-eval*")) (cur (current-buffer)))
      (unwind-protect
          (progn (with-current-buffer buf 'inner)
                 (eq cur (current-buffer)))
        (kill-buffer buf)))
    t)
(qa (let ((buf (generate-new-buffer " *qa-eval*")))
      (unwind-protect
          (with-current-buffer buf
            (insert "abcdef")
            (list (save-restriction
                    (narrow-to-region 2 4)
                    (list (point-min) (point-max)))
                  (point-min) (point-max)))
        (kill-buffer buf)))
    '((2 4) 1 7))
(qa (with-temp-buffer (insert "hi") (buffer-string)) "hi")

;; GH #132: interpreted min arity is 1; byte compilation folds the empty form.
(qa-err-data (unwind-protect) 'wrong-number-of-arguments '(unwind-protect 0))
(qa (unwind-protect nil) nil)
(qa (unwind-protect 42) 42)
(qa (let ((cleaned nil)) (list (unwind-protect 7 (setq cleaned t)) cleaned)) '(7 t))
(qa (let ((cleaned nil))
      (list (catch 'escape (unwind-protect (throw 'escape 9) (setq cleaned t))) cleaned)) '(9 t))
(qa-err-data (unwind-protect nil (error "cleanup")) 'error '("cleanup"))
(qa-err-data (unwind-protect (error "body") (error "cleanup")) 'error '("cleanup"))
(qa (func-arity 'unwind-protect) '(1 . unevalled))
(qa (subr-arity (symbol-function 'unwind-protect)) '(1 . unevalled))
(require 'bytecomp)
(qa (let ((byte-compile-warnings nil))
      (funcall (byte-compile '(lambda () (unwind-protect))))) nil)

(princ (qa--done))
