;;; -*- lexical-binding: nil; -*-
;; GNU 31.1: unused macro expansion environments are not validated (#293).
(require (quote macroexp))

;; macroexpand
(qa (macroexpand nil "bad") nil)
(qa (macroexpand t "bad") t)
(qa (macroexpand (quote x) "bad") (quote x))
(qa (macroexpand 42 "bad") 42)
(qa (macroexpand 3.14 "bad") 3.14)
(qa (macroexpand "x" "bad") "x")
(qa (macroexpand [x] "bad") [x])
(qa (macroexpand "x" 5) "x")
(qa (macroexpand "x" [bad]) "x")
(qa (macroexpand "x" (quote bad)) "x")
(qa (let ((form (propertize "x" (quote face) (quote bold)))) (eq form (macroexpand form "bad"))) t)
(qa-err (macroexpand (quote (when t 1)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand (quote (if t 1 2)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand (quote (lambda (x) x)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand (quote (nil)) "bad") (quote wrong-type-argument))

;; macroexpand-1
(qa (macroexpand-1 nil "bad") nil)
(qa (macroexpand-1 t "bad") t)
(qa (macroexpand-1 (quote x) "bad") (quote x))
(qa (macroexpand-1 42 "bad") 42)
(qa (macroexpand-1 3.14 "bad") 3.14)
(qa (macroexpand-1 "x" "bad") "x")
(qa (macroexpand-1 [x] "bad") [x])
(qa (macroexpand-1 "x" 5) "x")
(qa (macroexpand-1 "x" [bad]) "x")
(qa (macroexpand-1 "x" (quote bad)) "x")
(qa (let ((form (propertize "x" (quote face) (quote bold)))) (eq form (macroexpand-1 form "bad"))) t)
(qa-err (macroexpand-1 (quote (when t 1)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-1 (quote (if t 1 2)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-1 (quote (lambda (x) x)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-1 (quote (nil)) "bad") (quote wrong-type-argument))

;; macroexpand-all
(qa (macroexpand-all nil "bad") nil)
(qa (macroexpand-all t "bad") t)
(qa (macroexpand-all (quote x) "bad") (quote x))
(qa (macroexpand-all 42 "bad") 42)
(qa (macroexpand-all 3.14 "bad") 3.14)
(qa (macroexpand-all "x" "bad") "x")
(qa (macroexpand-all [x] "bad") [x])
(qa (macroexpand-all "x" 5) "x")
(qa (macroexpand-all "x" [bad]) "x")
(qa (macroexpand-all "x" (quote bad)) "x")
(qa (let ((form (propertize "x" (quote face) (quote bold)))) (eq form (macroexpand-all form "bad"))) t)
(qa-err (macroexpand-all (quote (when t 1)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-all (quote (if t 1 2)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-all (quote (lambda (x) x)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-all (quote (nil)) "bad") (quote wrong-type-argument))

(qa (macroexpand (quote (1 t)) "bad") (quote (1 t)))
(qa-err (macroexpand-1 (quote (1 t)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-all (quote (1 t)) "bad") (quote wrong-type-argument))
(qa (macroexpand (quote ([x])) "bad") (quote ([x])))
(qa-err (macroexpand-1 (quote ([x])) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-all (quote ([x])) "bad") (quote wrong-type-argument))
(qa (macroexpand (quote ((lambda (x) x) 1)) "bad") (quote ((lambda (x) x) 1)))
(qa-err (macroexpand-1 (quote ((lambda (x) x) 1)) "bad") (quote wrong-type-argument))
(qa-err (macroexpand-all (quote ((lambda (x) x) 1)) "bad") (quote wrong-type-argument))

;; GNU signals when walking a circular macroexpansion environment.
(qa (let ((env (list (cons 'not-a-macro nil))))
      (setcdr env env)
      (condition-case err
          (macroexpand-all '(ordinary-form) env)
        (circular-list (car err))))
    'circular-list)
(qa (let ((env (list (cons 'qa-macro-env-unrelated nil))))
      (setcdr env env)
      (condition-case err
          (macroexpand-1 '(qa-macro-env-form) env)
        (circular-list (car err))))
    'circular-list)

;; A matching environment entry returns before a circular tail is reached.
(qa (let ((env (list (cons 'qa-macro-env-form nil))))
      (setcdr env env)
      (macroexpand-1 '(qa-macro-env-form) env))
    '(qa-macro-env-form))

;; Improper environments signal only when lookup reaches the bad tail, and
;; all three public entry points report the original environment as data.
(dolist (function '(macroexpand-1 macroexpand macroexpand-all))
  (qa-err-data (funcall function '(qa-macro-env-form)
                        '((qa-macro-env-unrelated) . dotted-tail))
               'wrong-type-argument
               '(listp ((qa-macro-env-unrelated) . dotted-tail))))

;; Differential corpus case: a self-evaluating format string leaves the
;; unused, path-like macro environment uninspected by every expansion path.
(qa (macroexpand "%-10s|%05.2f" "file-a.txt") "%-10s|%05.2f")
(qa (macroexpand-1 "%-10s|%05.2f" "file-a.txt") "%-10s|%05.2f")
(qa (macroexpand-all "%-10s|%05.2f" "file-a.txt") "%-10s|%05.2f")
(qa (macroexpand--all-toplevel "%-10s|%05.2f" "file-a.txt") "%-10s|%05.2f")
(qa (macroexp-macroexpand "%-10s|%05.2f" "file-a.txt") "%-10s|%05.2f")

;; A self-referential CL symbol macro expands to itself and terminates.
(qa (progn
      (require 'cl-lib)
      (macroexpand-all
       '(cl-symbol-macrolet ((qa-self-symbol-macro qa-self-symbol-macro))
          qa-self-symbol-macro)))
    'qa-self-symbol-macro)

;; Recursive symbol-macro expansions are bounded by the Lisp evaluation
;; depth limit, including when a substitution contains the symbol again.
(qa (let ((max-lisp-eval-depth 100))
      (require 'cl-lib)
      (condition-case err
          (progn
            (macroexpand-all
             '(cl-symbol-macrolet ((qa-recursive-symbol-macro
                                    (list qa-recursive-symbol-macro)))
                qa-recursive-symbol-macro))
            'returned)
        (excessive-lisp-nesting (car err))))
    'excessive-lisp-nesting)

(princ (qa--done))
