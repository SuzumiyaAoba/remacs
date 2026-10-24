;;; Freeze the selected catcher before Lisp cleanup begins. -*- lexical-binding: t; -*-
(defvar qa-st-mode)
(defvar qa-st-events)
(defvar qa-st-conditions)
(defvar qa-st-handlers)
(defvar qa-st-missing (make-symbol "missing-handler"))
(defun qa-st-run (form compiled)
  (let (qa-st-events)
    (if compiled (funcall (byte-compile (list 'lambda nil form))) (eval form t))))

(defun qa-st-table (compiled action)
  (let* ((clause (list (if (eq action 'error) 'error :success) ''selected))
         (handlers (list clause))
         (body (cond
                ((eq action 'error)
                 '(unwind-protect (error "body")
                    (setcar qa-st-handlers '(error 'replacement))))
                ((eq action 'head)
                 '(progn (setcar (car qa-st-handlers) 'error) 'body))
                (t '(progn (setcar qa-st-handlers '(:success 'replacement)) 'body))))
         (protected (cons 'condition-case (cons nil (cons body handlers)))))
    (qa-st-run `(let ((qa-st-handlers ',handlers)) ,protected) compiled)))

(dolist (qa-st-mode '(nil t))
  ;; Both clauses belong to the same interpreted condition-case scope.
  (qa (qa-st-run
       '(progn
          (put 'qa-st-class 'error-conditions (list 'qa-st-class 'error))
          (condition-case nil
              (unwind-protect (signal 'qa-st-class nil)
                (setcar (cdr (get 'qa-st-class 'error-conditions)) 'file-error))
            (file-error 'wrong-clause) (error 'selected-clause))) qa-st-mode)
      'selected-clause)
  ;; Mutating a handler spec itself cannot change an already chosen clause.
  (qa (qa-st-run
       '(let ((qa-st-conditions '#1=(error)))
          (setcar qa-st-conditions 'error)
          (condition-case nil
              (unwind-protect (error "body") (setcar qa-st-conditions 'file-error))
            (#1# 'selected-clause) (error 'wrong-clause))) qa-st-mode)
      'selected-clause)
  ;; A new condition raised by an observer skips the conditions between that
  ;; observer and the original signal site, just as GNU's SKIP_CONDITIONS does.
  (qa (qa-st-run
       '(condition-case nil
            (handler-bind ((error (lambda (_) (signal 'file-error '("observer")))))
              (condition-case nil (error "body") (file-error 'stolen)))
          (file-error 'outer-handler)) qa-st-mode)
      'outer-handler)
  (qa (qa-st-run
       '(condition-case caught
            (handler-bind ((error qa-st-missing))
              (condition-case nil (error "body") (void-function 'stolen)))
          (void-function (eq (cadr caught) qa-st-missing))) qa-st-mode) t)
  ;; A previously rejected inner case is not reconsidered after an observer
  ;; destructively edits the condition list. Later observers see the edit.
  (qa (qa-st-run
       '(progn
          (put 'qa-st-class 'error-conditions (list 'qa-st-class 'error))
          (condition-case nil
              (handler-bind
               ((error (lambda (_)
                         (push 'first qa-st-events)
                         (setcar (cdr (get 'qa-st-class 'error-conditions)) 'file-error)))
                (file-error (lambda (_) (push 'second qa-st-events))))
               (condition-case nil (signal 'qa-st-class nil) (file-error 'stolen)))
            (file-error (list 'outer-handler (nreverse qa-st-events))))) qa-st-mode)
      '(outer-handler (first second)))
  (qa (qa-st-run
       '(condition-case nil
            (handler-bind ((error (lambda (_)
                                   (garbage-collect)
                                   (signal 'file-error '("observer")))))
              (condition-case nil (error "body") (file-error 'stolen)))
          (file-error 'outer-handler)) qa-st-mode)
      'outer-handler)
  ;; Separate in-flight packets keep their own selected destinations.
  (qa (qa-st-run
       '(condition-case caught
            (unwind-protect (error "body")
              (condition-case nil (signal 'file-error '("cleanup")) (file-error nil)))
          (error (cadr caught))) qa-st-mode)
      "body")
  (qa (qa-st-run
       '(progn
          (put 'qa-st-class 'error-conditions (list 'qa-st-class 'error))
          (condition-case nil
              (unwind-protect (signal 'qa-st-class nil)
                (thread-join
                 (make-thread
                  (lambda ()
                    (setcar (cdr (get 'qa-st-class 'error-conditions)) 'file-error)))))
            (file-error 'wrong-clause) (error 'selected-clause))) qa-st-mode)
      'selected-clause))

(dolist (qa-st-mode '(nil t))
  (qa (qa-st-table qa-st-mode 'error) 'selected)
  (qa (qa-st-table qa-st-mode 'success) 'selected)
  (qa (qa-st-table qa-st-mode 'head) 'selected)
  (qa (qa-st-run '(condition-case result 'body
                    (:success (list 'first result))
                    (:success (list 'last result))) qa-st-mode)
      (if qa-st-mode '(first body) '(last body))))

(princ (qa--done))
