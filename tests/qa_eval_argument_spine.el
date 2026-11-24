;;; -*- lexical-binding: t; -*-
;; GNU snapshots the next argument cell before evaluating the current one.
(defvar qa-eval-spine-form nil)
(defvar qa-eval-dotted-arg-effect nil)
(defvar qa-eval-dotted-autoload-loaded nil)
(defvar qa-eval-circular-unbound-side nil)
(defun qa-eval-dotted-one-arg (arg) arg)

(defun qa-eval-spine-replace-tail ()
  (setcdr (cdr qa-eval-spine-form) '((quote added)))
  'first)

(qa (let ((qa-eval-spine-form
           (list 'list '(qa-eval-spine-replace-tail))))
      (eval qa-eval-spine-form))
    '(first))

(defun qa-eval-spine-change-next-car ()
  (setcar (cddr qa-eval-spine-form) '(quote changed))
  'first)

(qa (let ((qa-eval-spine-form
           (list 'list '(qa-eval-spine-change-next-car) '(quote original))))
      (eval qa-eval-spine-form))
    '(first changed))

(defun qa-eval-spine-change-next-cdr ()
  (setcdr (cddr qa-eval-spine-form) '((quote added)))
  'first)

(qa (let ((qa-eval-spine-form
           (list 'list '(qa-eval-spine-change-next-cdr) '(quote original))))
      (eval qa-eval-spine-form))
    '(first original))

;; `cond' reads the selected clause body after evaluating its test. A test
;; that removes the body leaves the test value as the clause result.
(defvar qa-eval-cond-clause nil)
(defun qa-eval-cond-remove-body ()
  (setcdr qa-eval-cond-clause nil)
  t)
(qa (let* ((qa-eval-cond-clause
            (list '(qa-eval-cond-remove-body) '(quote original))))
      (eval (list 'cond qa-eval-cond-clause)))
    t)

;; Circular argument spines signal before evaluating any argument forms.
(defvar qa-eval-spine-circular-side nil)
(defun qa-eval-spine-circular-function (&rest args) args)
(qa (let* ((args (list '(setq qa-eval-spine-circular-side t)))
           (form (cons 'qa-eval-spine-circular-function args)))
      (setcdr args args)
      (condition-case err (eval form) (error (car err))))
    'circular-list)
(qa qa-eval-spine-circular-side nil)
(qa (let* ((args (list '(setq qa-eval-spine-circular-side t) 'a 'b))
           (cycle (cdr args))
           (form (cons 'qa-eval-spine-circular-function args)))
      (setcdr (cdr cycle) cycle)
      (setq qa-eval-spine-circular-side nil)
      (condition-case err
          (eval form)
        (circular-list
         (list (eq (cadr err) args)
               (eq (cadr err) cycle)
               (eq (cadr err) (cdr cycle))
               qa-eval-spine-circular-side))))
    '(nil nil t nil))
(qa (let* ((args (list '(setq qa-eval-spine-circular-side t) 'a 'b))
           (cycle (cdr args))
           (form (cons '(lambda (&rest args) args) args)))
      (setcdr (cdr cycle) cycle)
      (setq qa-eval-spine-circular-side nil)
      (condition-case err
          (eval form)
        (circular-list
         (list (eq (cadr err) args)
               (eq (cadr err) cycle)
               (eq (cadr err) (cdr cycle))
               qa-eval-spine-circular-side))))
    '(nil nil t nil))
(qa (let* ((args (list '(setq qa-eval-circular-unbound-side t)))
           (form (cons 'qa-eval-circular-unbound args)))
      (setcdr args args)
      (condition-case err
          (eval form)
        (void-function
         (list (car err) (cadr err) qa-eval-circular-unbound-side))))
    '(void-function qa-eval-circular-unbound nil))
(qa (let* ((args (list 1)) (form (cons 5 args)))
      (setcdr args args)
      (condition-case err
          (eval form)
        (invalid-function (list (car err) (cadr err)))))
    '(invalid-function 5))
(qa (condition-case err
        (eval '((autoload "qa-no-such-file" nil nil nil) 1))
      (wrong-type-argument (list (car err) (cadr err) (caddr err))))
    '(wrong-type-argument symbolp
      (autoload "qa-no-such-file" nil nil nil)))

(defvar qa-eval-lambda-cycle-bcell nil)
(defvar qa-eval-lambda-cycle-count 0)
(qa (progn
      (setq qa-eval-lambda-cycle-count 0
            qa-eval-lambda-cycle-bcell nil)
      (fset 'qa-eval-lambda-cycle-watch-a
            (make-interpreted-closure
             '(_symbol _value operation _where)
             '((if (eq operation 'let)
                   (setcdr qa-eval-lambda-cycle-bcell
                           qa-eval-lambda-cycle-bcell)))
             nil))
      (fset 'qa-eval-lambda-cycle-watch-b
            (make-interpreted-closure
             '(_symbol _value operation _where)
             '((if (eq operation 'let)
                   (progn
                     (setq qa-eval-lambda-cycle-count
                           (1+ qa-eval-lambda-cycle-count))
                     (if (= qa-eval-lambda-cycle-count 2)
                         (setcdr qa-eval-lambda-cycle-bcell nil)))))
             nil))
      (let* ((arglist (list 'qa-eval-lambda-cycle-a
                            '&optional
                            'qa-eval-lambda-cycle-b))
             (qa-eval-lambda-cycle-bcell (cddr arglist))
             (function
              (make-interpreted-closure arglist
                                        '(qa-eval-lambda-cycle-b) nil)))
        (unwind-protect
            (progn
              (add-variable-watcher 'qa-eval-lambda-cycle-a
                                    'qa-eval-lambda-cycle-watch-a)
              (add-variable-watcher 'qa-eval-lambda-cycle-b
                                    'qa-eval-lambda-cycle-watch-b)
              (list (condition-case err
                        (funcall function 'first)
                      (error (list :error (car err))))
                    qa-eval-lambda-cycle-count))
          (remove-variable-watcher 'qa-eval-lambda-cycle-a
                                   'qa-eval-lambda-cycle-watch-a)
          (remove-variable-watcher 'qa-eval-lambda-cycle-b
                                   'qa-eval-lambda-cycle-watch-b))))
    '(nil 2))

;; Autoload code can mutate the caller's form before GNU recomputes the
;; function-call argument count. The new circular spine signals before any
;; argument form runs; the pre-load count must not drive the later walk.
(defvar qa-eval-autoload-mutated-form nil)
(defvar qa-eval-autoload-argument-ran nil)
(qa
 (let* ((file (make-temp-file "qa-eval-autoload-mutates-args" nil ".el"))
        (qa-eval-autoload-mutated-form nil)
        (qa-eval-autoload-argument-ran nil)
        (result nil))
   (unwind-protect
       (let* ((args (list '(setq qa-eval-autoload-argument-ran t)))
              (form (cons 'qa-eval-autoload-mutates-args args)))
         (setq qa-eval-autoload-mutated-form form)
         (with-temp-file file
           (insert "(setcdr (cdr qa-eval-autoload-mutated-form) \
                           (cdr qa-eval-autoload-mutated-form))\n"
                   "(defun qa-eval-autoload-mutates-args (&rest args) args)\n"))
         (fmakunbound 'qa-eval-autoload-mutates-args)
         (autoload 'qa-eval-autoload-mutates-args file nil nil)
         (condition-case err
             (eval form)
           (circular-list
            (setq result
                  (list (car err) qa-eval-autoload-argument-ran))))
         result)
     (ignore-errors (delete-file file))
     (fmakunbound 'qa-eval-autoload-mutates-args)))
 '(circular-list nil))

;; Macro expansion also receives raw forms, but GNU reports the cyclic
;; argument spine with `circular-list' before invoking the expander.
(defvar qa-eval-spine-circular-macro-side nil)
(defmacro qa-eval-spine-circular-macro (&rest args)
  (setq qa-eval-spine-circular-macro-side 'expanded)
  nil)
(qa (let* ((args (list '(setq qa-eval-spine-circular-macro-side 'evaluated)))
           (form (cons 'qa-eval-spine-circular-macro args)))
      (setcdr args args)
      (setq qa-eval-spine-circular-macro-side nil)
      (condition-case err
          (eval form)
        (circular-list (list (car err) (eq (cadr err) args)
                             qa-eval-spine-circular-macro-side))))
    '(circular-list t nil))
(qa (let* ((args (list 'raw))
           (form (cons 'qa-eval-spine-circular-macro args)))
      (setcdr args args)
      (condition-case err
          (macroexpand-1 form)
        (circular-list (list (car err) (eq (cadr err) args)))))
    '(circular-list t))
(qa (let* ((args (list 'raw))
           (form (cons 'qa-eval-spine-circular-macro args)))
      (setcdr args args)
      (condition-case err
          (macroexpand form)
        (circular-list (list (car err) (eq (cadr err) args)))))
    '(circular-list t))
(qa (let ((form (cons 'qa-eval-spine-circular-macro (cons 'raw 'tail))))
      (setq qa-eval-spine-circular-macro-side nil)
      (condition-case err
          (eval form)
        (wrong-type-argument
         (list (car err) (cadr err) (caddr err)
               qa-eval-spine-circular-macro-side))))
    '(wrong-type-argument listp tail nil))
(qa (let ((form (cons 'qa-eval-spine-circular-macro (cons 'raw 'tail))))
      (setq qa-eval-spine-circular-macro-side nil)
      (condition-case err
          (macroexpand-1 form)
        (wrong-type-argument
         (list (car err) (cadr err) (caddr err)
               qa-eval-spine-circular-macro-side))))
    '(wrong-type-argument listp tail nil))
(qa (let ((form (cons 'qa-eval-spine-circular-macro (cons 'raw 'tail))))
      (setq qa-eval-spine-circular-macro-side nil)
      (condition-case err
          (macroexpand form)
        (wrong-type-argument
         (list (car err) (cadr err) (caddr err)
               qa-eval-spine-circular-macro-side))))
    '(wrong-type-argument listp tail nil))
(defvar qa-eval-macro-repair-count 0)
(defvar qa-eval-macro-repair-target nil)
(qa (progn
      (setq qa-eval-macro-repair-count 0
            qa-eval-macro-repair-target nil)
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-macro-trigger
            (cons 'macro
                  (make-interpreted-closure nil
                    '((setcdr qa-eval-macro-repair-target
                              qa-eval-macro-repair-target)
                      nil)
                    nil)))
      (fset 'qa-eval-macro-repair
            (cons 'macro
                  (make-interpreted-closure nil
                    '((setq qa-eval-macro-repair-count
                            (1+ qa-eval-macro-repair-count))
                      (if (= qa-eval-macro-repair-count 2)
                          (setcdr qa-eval-macro-repair-target nil))
                      nil)
                    nil)))
      (let* ((target (list '(qa-eval-macro-repair)))
             (form (cons 'list (cons '(qa-eval-macro-trigger) target))))
        (setq qa-eval-macro-repair-target target)
        (list (condition-case err
                  (progn (macroexpand-all form) 'ok)
                (error (list :error err)))
              qa-eval-macro-repair-count
              (eq (cdr target) target))))
    '(ok 2 nil))
(qa (progn
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-dotted-handler-macro (cons 'macro 'ignore))
      (macroexpand-all
       '(condition-case nil nil
          (error (qa-eval-dotted-handler-macro) . tail))))
    '(condition-case nil nil (error nil . tail)))
(qa (progn
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-dotted-defun-macro (cons 'macro 'ignore))
      (list (condition-case err
                (macroexpand-all
                 '(defun qa-eval-dotted-f nil
                    (qa-eval-dotted-defun-macro) . tail))
              (error err))
            (condition-case err
                (macroexpand-all
                 '(defmacro qa-eval-dotted-m nil
                    (qa-eval-dotted-defun-macro) . tail))
              (error err))))
    '((wrong-type-argument listp tail)
      (wrong-type-argument listp tail)))
(qa (progn
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-dotted-let-macro (cons 'macro 'ignore))
      (list (macroexpand-all
             '(let ((qa-eval-dotted-let-x (qa-eval-dotted-let-macro)))
                (qa-eval-dotted-let-macro) . tail))
            (macroexpand-all
             '(let* ((qa-eval-dotted-let-x (qa-eval-dotted-let-macro)))
                (qa-eval-dotted-let-macro) . tail))))
    '((let ((qa-eval-dotted-let-x nil)) nil . tail)
      (let* ((qa-eval-dotted-let-x nil)) nil . tail)))
(qa (progn
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-dotted-condition-var-macro (cons 'macro 'ignore))
      (macroexpand-all
       '(condition-case (qa-eval-dotted-condition-var-macro)
          nil
          (error (qa-eval-dotted-condition-var-macro))
          . tail)))
    '(condition-case (qa-eval-dotted-condition-var-macro)
       nil (error nil) . tail))
(qa (progn
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-dotted-funcall-macro (cons 'macro 'ignore))
      (macroexpand-all
       '(funcall (function list)
          (qa-eval-dotted-funcall-macro) . tail)))
    '(list nil . tail))
(qa (progn
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (defalias 'qa-eval-if-macroexpand-alias 'if)
      (fset 'qa-eval-real-macro
            (cons 'macro (make-interpreted-closure nil '(nil) nil)))
      (defalias 'qa-eval-macro-expand-alias 'qa-eval-real-macro)
      (list (macroexpand-all
             '(funcall (function qa-eval-if-macroexpand-alias) t 1 2))
            (macroexpand-all
             '(funcall (function qa-eval-macro-expand-alias)))))
    '((funcall #'qa-eval-if-macroexpand-alias t 1 2)
      (funcall #'qa-eval-macro-expand-alias)))
(defvar qa-eval-funcarg-cycle-count 0)
(qa (progn
      (setq qa-eval-funcarg-cycle-count 0)
      (if (fboundp 'macroexpand-all) nil (require 'macroexp))
      (fset 'qa-eval-funcarg-cycle-macro
            (cons 'macro
                  (make-interpreted-closure nil
                    '((setq qa-eval-funcarg-cycle-count
                            (1+ qa-eval-funcarg-cycle-count))
                      nil)
                    nil)))
      (let* ((arglist (list 'qa-eval-funcarg-cycle-var))
             (lambda-form
              (list 'lambda arglist '(qa-eval-funcarg-cycle-macro)))
             (form (list 'function lambda-form)))
        (setcdr arglist arglist)
        (list (condition-case err
                  (progn (macroexpand-all form) 'ok)
                (error (list :error (car err))))
              qa-eval-funcarg-cycle-count)))
    '(ok 1))

;; Circular malformed let specs signal promptly and retain the original
;; cyclic spec as one error datum instead of flattening it forever.
(qa (let ((spec (list 'x)))
      (setcdr spec spec)
      (condition-case err
          (eval (list 'let (list spec) 'body))
        (error (list (car err) (eq (caddr err) spec)))))
    '(error t))

;; Required circular lambda parameters consume the supplied arguments, then
;; signal wrong-number-of-arguments when the next cycle needs another value.
(qa (let ((args (list 'x)))
      (setcdr args args)
      (condition-case err
          (funcall (list 'lambda args 'x) 1)
        (wrong-number-of-arguments (car err))))
    'wrong-number-of-arguments)

;; Binding a raw lambda parameter runs variable watchers. GNU advances from
;; the live arglist cdr afterward, so mutations from the watcher affect the
;; remaining parameter list.
(qa (let* ((arglist (list 'qa-eval-lambda-first 'qa-eval-lambda-second))
           (form (list 'lambda arglist
                       '(list qa-eval-lambda-first qa-eval-lambda-second)))
           (watcher
            (lambda (_symbol _value operation _where)
              (when (eq operation 'let)
                (setcdr arglist nil)))))
      (unwind-protect
          (progn
            (add-variable-watcher 'qa-eval-lambda-first watcher)
            (condition-case err
                (funcall form 'first 'second)
              (wrong-number-of-arguments
               (list (car err) (equal (cadr err) form) (caddr err)))))
        (remove-variable-watcher 'qa-eval-lambda-first watcher)))
    '(wrong-number-of-arguments t 2))

;; The same live-cdr rule applies to an interpreted closure's raw arglist.
(qa (let* ((arglist (list 'qa-eval-closure-first 'qa-eval-closure-second))
           (function
            (make-interpreted-closure
             arglist
             '((list qa-eval-closure-first qa-eval-closure-second))
             nil))
           (watcher
            (lambda (_symbol _value operation _where)
              (when (eq operation 'let)
                (setcdr arglist nil)))))
      (unwind-protect
          (progn
            (add-variable-watcher 'qa-eval-closure-first watcher)
            (condition-case err
                (funcall function 'first 'second)
              (wrong-number-of-arguments
               (list (car err) (eq (cadr err) function) (caddr err)))))
        (remove-variable-watcher 'qa-eval-closure-first watcher)))
    '(wrong-number-of-arguments t 2))

;; A watcher can replace a raw lambda's body while its parameters bind.
;; GNU reads XCDR(XCDR(FUN)) after that binding loop.
(qa (let* ((arglist (list 'qa-eval-lambda-body-parameter))
           (form (list 'lambda arglist '(quote old)))
           (watcher
            (lambda (_symbol _value operation _where)
              (when (eq operation 'let)
                (setcdr form (cons arglist '((quote new))))))))
      (unwind-protect
          (progn
            (add-variable-watcher 'qa-eval-lambda-body-parameter watcher)
            (funcall form 'argument))
        (remove-variable-watcher 'qa-eval-lambda-body-parameter watcher)))
    'new)

;; Lambda construction accepts malformed optional parameters; the raw
;; argument walker reports invalid-function when the resulting closure runs.
(qa (let ((function (eval '(function (lambda (&optional 5) 'body)) nil)))
      (condition-case err
          (funcall function)
        (invalid-function (eq (cadr err) function))))
    t)
(qa (let ((function (eval '(function (lambda (&optional (5)) 'body)) nil)))
      (condition-case err
          (funcall function)
        (invalid-function (eq (cadr err) function))))
    t)

;; A trailing &optional marker is valid: it introduces no slots. Calls with
;; an argument still fail the ordinary arity check.
(qa (let ((function (eval '(function (lambda (&optional) 'body)) nil)))
      (list (funcall function)
            (condition-case err
                (funcall function 'extra)
              (wrong-number-of-arguments (car err)))))
    '(body wrong-number-of-arguments))

;; Interpreted lambda calls reject malformed marker sequences after
;; construction, while a dotted tail is checked after required arguments.
(defun qa-eval-arglist-result (arglist arguments)
  (let ((function (make-interpreted-closure arglist '(t) nil)))
    (condition-case err
        (apply function arguments)
      (error (car err)))))
(qa (mapcar (lambda (arglist)
              (qa-eval-arglist-result arglist nil))
            '((&rest)
              (&optional x &optional y)
              (&rest x &rest y)
              (&rest x &optional y)
              (&optional x &rest y &optional z)))
    '(invalid-function invalid-function invalid-function
      invalid-function invalid-function))
(qa (list (qa-eval-arglist-result '(x . tail) nil)
          (qa-eval-arglist-result '(x . tail) '(value)))
    '(wrong-number-of-arguments invalid-function))

;; GNU `lambda_arity' stops at the first &rest marker and does not inspect
;; malformed parameters or tails that follow it.
(qa (func-arity '(lambda (x &rest 5) x)) '(1 . many))
(qa (func-arity '(lambda (x &rest . tail) x)) '(1 . many))

;; `defun' validates its argument list with `mapcar', which signals on a
;; circular spine instead of looping while checking each parameter.
(qa (let ((args (list 'x)))
      (setcdr args args)
      (condition-case err
          (eval (list 'defun 'qa-eval-spine-circular-defun args 't))
        (circular-list (car err))))
    'circular-list)

;; `mapcar' in the GNU defun arglist check traverses past a nonsymbol so a
;; later dotted tail still signals its listp error before malformed-arglist.
(qa (condition-case err
        (eval '(defun qa-eval-spine-dotted-defun (x 5 . tail) t))
      (wrong-type-argument (list (car err) (cadr err) (caddr err))))
    '(wrong-type-argument listp tail))

;; Parallel `let' rejects the circular varlist before any initializer;
;; sequential `let*' has evaluated the first binding before it reaches the
;; repeated cell.
(defvar qa-eval-circular-let-count nil)
(qa (let ((bindings
           (list (list 'x '(setq qa-eval-circular-let-count 'ran)))))
      (setcdr bindings bindings)
      (setq qa-eval-circular-let-count nil)
      (condition-case err
          (eval (list 'let bindings nil))
        (circular-list (list (car err) qa-eval-circular-let-count))))
    '(circular-list nil))
(qa (let ((bindings
           (list (list 'x '(setq qa-eval-circular-let-count 'ran)))))
      (setcdr bindings bindings)
      (setq qa-eval-circular-let-count nil)
      (condition-case err
          (eval (list 'let* bindings nil))
        (circular-list (list (car err) qa-eval-circular-let-count))))
    '(circular-list ran))
(defvar qa-eval-letstar-repair-count 0)
(qa (progn
      (setq qa-eval-letstar-repair-count 0)
      (let* ((second-cell (cons nil nil))
             (first-init
              (list 'setcdr
                    (list 'quote second-cell)
                    (list 'quote second-cell)))
             (second-init
              (list 'progn
                    (list 'setq 'qa-eval-letstar-repair-count
                          (list '1+ 'qa-eval-letstar-repair-count))
                    (list 'if
                          (list '= 'qa-eval-letstar-repair-count 2)
                          (list 'setcdr (list 'quote second-cell) nil))
                    nil))
             (first-spec (list 'qa-eval-letstar-a first-init))
             (second-spec (list 'qa-eval-letstar-b second-init))
             (bindings (cons first-spec second-cell))
             (form (list 'let* bindings 'qa-eval-letstar-repair-count)))
        (setcar second-cell second-spec)
        (list (condition-case err
                  (eval form nil)
                (error (list :error (car err))))
              qa-eval-letstar-repair-count)))
    '(2 2))

;; Circular `setq' arguments are rejected before any RHS is evaluated.
(defvar qa-eval-circular-setq-side-effect nil)
(qa (let ((args
           (list 'qa-eval-circular-setq-side-effect
                 '(setq qa-eval-circular-setq-side-effect 'ran))))
      (setcdr (cdr args) args)
      (setq qa-eval-circular-setq-side-effect nil)
      (condition-case err
          (eval (cons 'setq args))
        (circular-list
         (list (car err) qa-eval-circular-setq-side-effect))))
    '(circular-list nil))
(defvar qa-eval-setq-repair-count 0)
(qa (progn
      (setq qa-eval-setq-repair-count 0)
      (let* ((rhs2-cell (cons nil nil))
             (y-cell (cons 'qa-eval-setq-y rhs2-cell))
             (rhs1-cell (cons nil y-cell))
             (x-cell (cons 'qa-eval-setq-x rhs1-cell))
             (rhs1-form
              (list 'setcdr
                    (list 'quote rhs2-cell)
                    (list 'quote y-cell)))
             (rhs2-form
              (list 'progn
                    (list 'setq 'qa-eval-setq-repair-count
                          (list '1+ 'qa-eval-setq-repair-count))
                    (list 'if
                          (list '= 'qa-eval-setq-repair-count 2)
                          (list 'setcdr (list 'quote rhs2-cell) nil))
                    nil))
             (form (cons 'setq x-cell)))
        (setcar rhs1-cell rhs1-form)
        (setcar rhs2-cell rhs2-form)
        (list (condition-case err
                  (eval form nil)
                (error (list :error (car err))))
              qa-eval-setq-repair-count)))
    '(nil 3))

;; A circular progn body is rejected before its first form runs.
(defvar qa-eval-circular-progn-side-effect nil)
(qa (let ((forms (list '(setq qa-eval-circular-progn-side-effect 'ran))))
      (setcdr forms forms)
      (setq qa-eval-circular-progn-side-effect nil)
      (condition-case err
          (eval (cons 'progn forms))
        (circular-list
         (list (car err) qa-eval-circular-progn-side-effect))))
    '(circular-list nil))

;; `and'/`or'/`cond' reject a circular outer argument spine before
;; evaluating a form that would otherwise short-circuit.
(defvar qa-eval-circular-short-circuit-side nil)
(qa (let ((forms
           (list '(progn (setq qa-eval-circular-short-circuit-side 'ran) nil))))
      (setcdr forms forms)
      (setq qa-eval-circular-short-circuit-side nil)
      (condition-case err
          (eval (cons 'and forms))
        (circular-list (list (car err) qa-eval-circular-short-circuit-side))))
    '(circular-list nil))
(qa (let ((forms
           (list '(progn (setq qa-eval-circular-short-circuit-side 'ran) t))))
      (setcdr forms forms)
      (setq qa-eval-circular-short-circuit-side nil)
      (condition-case err
          (eval (cons 'or forms))
        (circular-list (list (car err) qa-eval-circular-short-circuit-side))))
    '(circular-list nil))
(qa (let ((clauses
           (list (list '(progn (setq qa-eval-circular-short-circuit-side 'ran) t)))))
      (setcdr clauses clauses)
      (setq qa-eval-circular-short-circuit-side nil)
      (condition-case err
          (eval (cons 'cond clauses))
        (circular-list (list (car err) qa-eval-circular-short-circuit-side))))
    '(circular-list nil))
(defvar qa-eval-or-repair-count 0)
(qa (progn
      (setq qa-eval-or-repair-count 0)
      (let* ((second-cell (cons nil nil))
             (first-form
              (list 'progn
                    (list 'setcdr
                          (list 'quote second-cell)
                          (list 'quote second-cell))
                    nil))
             (second-form
              (list 'progn
                    (list 'setq 'qa-eval-or-repair-count
                          (list '1+ 'qa-eval-or-repair-count))
                    (list 'if
                          (list '= 'qa-eval-or-repair-count 1)
                          (list 'setcdr (list 'quote second-cell) nil))
                    nil))
             (form (cons 'or (cons first-form second-cell))))
        (setcar second-cell second-form)
        (list (condition-case err
                  (eval form nil)
                (error (list :error (car err))))
              qa-eval-or-repair-count)))
    '(nil 2))
(defvar qa-eval-cond-repair-count 0)
(qa (progn
      (setq qa-eval-cond-repair-count 0)
      (let* ((second-cell (cons nil nil))
             (first-test
              (list 'progn
                    (list 'setcdr
                          (list 'quote second-cell)
                          (list 'quote second-cell))
                    nil))
             (second-test
              (list 'progn
                    (list 'setq 'qa-eval-cond-repair-count
                          (list '1+ 'qa-eval-cond-repair-count))
                    (list 'if
                          (list '= 'qa-eval-cond-repair-count 2)
                          (list 'setcdr (list 'quote second-cell) nil))
                    nil))
             (first-clause (list first-test))
             (second-clause (list second-test))
             (form (cons 'cond (cons first-clause second-cell))))
        (setcar second-cell second-clause)
        (list (condition-case err
                  (eval form nil)
                (error (list :error (car err))))
              qa-eval-cond-repair-count)))
    '(nil 2))

;; Function-definition bodies must reject circular and dotted tails instead
;; of silently turning them into empty bodies.
(qa (let* ((body (list 42))
           (form (list 'defun 'qa-eval-circular-defun-body nil 42)))
      (setcdr (nthcdr 2 form) body)
      (setcdr body body)
      (condition-case err (eval form) (circular-list (car err))))
    'circular-list)
(qa (let* ((body-form (list 42))
           (form (list 'defun 'qa-eval-dotted-defun-body nil 42)))
      (setcdr (nthcdr 2 form) body-form)
      (setcdr body-form 'dotted-tail)
      (condition-case err (eval form) (wrong-type-argument (car err))))
    'wrong-type-argument)
(qa (let* ((body (list 42))
           (form (list 'defmacro 'qa-eval-circular-defmacro-body nil 42)))
      (setcdr (nthcdr 2 form) body)
      (setcdr body body)
      (condition-case err (eval form) (circular-list (car err))))
    'circular-list)

;; GNU resolves a symbol's function cell before rejecting dotted arguments:
;; an unbound head wins, while a resolved callable reports the bad tail.
(qa (condition-case err
        (eval (cons 'qa-eval-dotted-unbound 'tail))
      (wrong-type-argument (list (car err) (cadr err) (caddr err))))
    '(wrong-type-argument listp tail))
(qa (condition-case err
        (eval (cons 'qa-eval-dotted-unbound (cons 1 'tail)))
      (void-function (list (car err) (cadr err))))
    '(void-function qa-eval-dotted-unbound))
(qa (let ((qa-eval-dotted-arg-effect nil))
      (fmakunbound 'qa-eval-dotted-unbound)
      (condition-case err
          (eval (cons 'qa-eval-dotted-unbound
                      (cons '(setq qa-eval-dotted-arg-effect t) 'tail)))
        (void-function
         (list (car err) (cadr err) qa-eval-dotted-arg-effect))))
    '(void-function qa-eval-dotted-unbound nil))
(qa (let ((qa-eval-dotted-arg-effect nil))
      (unwind-protect
          (progn
            (fset 'qa-eval-dotted-nonfunction 5)
            (condition-case err
                (eval (cons 'qa-eval-dotted-nonfunction
                            (cons '(setq qa-eval-dotted-arg-effect t) 'tail)))
              (invalid-function
               (list (car err) (cadr err) qa-eval-dotted-arg-effect))))
        (fmakunbound 'qa-eval-dotted-nonfunction)))
    '(invalid-function qa-eval-dotted-nonfunction nil))
(qa (condition-case err
        (eval (cons 'qa-eval-dotted-one-arg (cons 1 (cons 2 'tail))))
      (wrong-type-argument
       (list (car err) (cadr err) (caddr err))))
    '(wrong-type-argument listp tail))
(qa (condition-case err
        (eval (cons '(lambda (x) x) (cons 1 'tail)))
      (wrong-type-argument
       (list (car err) (cadr err) (caddr err))))
    '(wrong-type-argument listp tail))
(qa (let ((qa-eval-dotted-arg-effect nil))
      (unwind-protect
          (progn
            (fset 'qa-eval-dotted-nil-function nil)
            (condition-case err
                (eval (cons 'qa-eval-dotted-nil-function 'tail))
              (wrong-type-argument
               (list (car err) (cadr err) (caddr err) qa-eval-dotted-arg-effect))))
        (fmakunbound 'qa-eval-dotted-nil-function)))
    '(wrong-type-argument listp tail nil))
(qa (let ((qa-eval-dotted-arg-effect nil))
      (unwind-protect
          (progn
            (fset 'qa-eval-dotted-nil-function nil)
            (condition-case err
                (eval (cons 'qa-eval-dotted-nil-function
                            (cons '(setq qa-eval-dotted-arg-effect t) 'tail)))
              (void-function
               (list (car err) (cadr err) qa-eval-dotted-arg-effect))))
        (fmakunbound 'qa-eval-dotted-nil-function)))
    '(void-function qa-eval-dotted-nil-function nil))
(qa (condition-case err
        (eval (cons nil (cons 1 'tail)))
      (void-function (list (car err) (cadr err))))
    '(void-function nil))
(qa (condition-case err
        (eval (cons 5 (cons 1 'tail)))
      (invalid-function (list (car err) (cadr err))))
    '(invalid-function 5))
(qa (let ((qa-eval-dotted-arg-effect nil))
      (condition-case err
          (eval (cons 'if
                      (cons '(setq qa-eval-dotted-arg-effect t)
                            (cons t 'tail))))
          (wrong-type-argument
         (list (car err) (cadr err) (caddr err) qa-eval-dotted-arg-effect)))
      )
    '(wrong-type-argument listp tail nil))
(qa (progn
      (fmakunbound 'qa-eval-dotted-alias)
      (fmakunbound 'qa-eval-dotted-alias-target)
      (fset 'qa-eval-dotted-alias 'qa-eval-dotted-alias-target)
      (condition-case err
          (eval (cons 'qa-eval-dotted-alias (cons 1 'tail)))
        (void-function (list (car err) (cadr err)))))
    '(void-function qa-eval-dotted-alias))
(qa (let* ((file (make-temp-file "qa-eval-dotted-autoload" nil ".el"))
           (qa-eval-dotted-autoload-loaded nil)
           (form (cons 'qa-eval-dotted-autoload (cons 1 'tail)))
           (result nil))
      (unwind-protect
          (progn
            (with-temp-file file
              (insert "(setq qa-eval-dotted-autoload-loaded t)\n"
                      "(defun qa-eval-dotted-autoload (&rest args) args)\n"))
            (fmakunbound 'qa-eval-dotted-autoload)
            (autoload 'qa-eval-dotted-autoload file nil nil)
            (condition-case err
                (eval form)
              (wrong-type-argument
               (setq result
                     (list (car err) (cadr err) (caddr err)
                           qa-eval-dotted-autoload-loaded))))
            result)
        (ignore-errors (delete-file file))
        (fmakunbound 'qa-eval-dotted-autoload)))
    '(wrong-type-argument listp tail t))

(princ (qa--done))
