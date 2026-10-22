;;; qa_handler_dynamic_order.el --- Respect handler-bind and condition-case nesting -*- lexical-binding: t; -*-

(defvar qa335-seen nil)
(defun qa335-order (form &optional compiled)
  (let ((qa335-seen nil))
    (list (if compiled (funcall (byte-compile (list 'lambda nil form))) (eval form t))
          (nreverse qa335-seen))))

;; outer handler cannot steal inner catch
(qa (qa335-order '(catch 'out (handler-bind ((error (lambda (_) (push 'outer qa335-seen) (throw 'out 'stolen)))) (condition-case nil (error "x") (error 'caught)))))
    '(caught nil))

;; outer observer is hidden by inner catch
(qa (qa335-order '(handler-bind ((error (lambda (_) (push 'outer qa335-seen)))) (condition-case nil (error "x") (error 'caught))))
    '(caught nil))

;; inner observer before outer catch
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'inner qa335-seen)))) (error "x")) (error 'caught)))
    '(caught (inner)))

;; resignal is offered once
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'observer qa335-seen)))) (condition-case err (error "x") (error (signal (car err) (cdr err))))) (error 'caught)))
    '(caught (observer)))

;; nonmatching inner case does not hide observer
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'observer qa335-seen)))) (condition-case nil (error "x") (file-error 'wrong))) (error 'caught)))
    '(caught (observer)))

;; observers follow dynamic nesting
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'outer qa335-seen)))) (condition-case nil (handler-bind ((error (lambda (_) (push 'inner qa335-seen)))) (error "x")) (file-error 'wrong))) (error 'caught)))
    '(caught (inner outer)))

;; same-call observers preserve argument order
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'first qa335-seen))) (error (lambda (_) (push 'second qa335-seen)))) (error "x")) (error 'caught)))
    '(caught (first second)))

;; observer nonlocal exit stops later observers
(qa (qa335-order '(catch 'out (handler-bind ((error (lambda (_) (push 'first qa335-seen) (throw 'out 'stopped))) (error (lambda (_) (push 'second qa335-seen)))) (error "x"))))
    '(stopped (first)))

;; unmatched observer is ignored
(qa (qa335-order '(condition-case nil (handler-bind ((file-error (lambda (_) (push 'wrong qa335-seen)))) (error "x")) (error 'caught)))
    '(caught nil))

;; condition lists match observers
(qa (qa335-order '(condition-case nil (handler-bind (((file-error arith-error) (lambda (_) (push 'arithmetic qa335-seen)))) (/ 1 0)) (arith-error 'caught)))
    '(caught (arithmetic)))

;; catch-all inner case hides outer observer
(qa (qa335-order '(handler-bind ((error (lambda (_) (push 'outer qa335-seen)))) (condition-case nil (error "x") (t 'caught))))
    '(caught nil))

;; quit observer is not error observer
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'wrong qa335-seen))) (quit (lambda (_) (push 'quit qa335-seen)))) (signal 'quit nil)) (quit 'caught)))
    '(caught (quit)))

;; inner catch nonlocal exit precedes outer handler
(qa (qa335-order '(catch 'out (handler-bind ((error (lambda (_) (push 'wrong qa335-seen)))) (condition-case nil (error "x") (error (throw 'out 'caught))))))
    '(caught nil))

;; observer binding is gone when its surrounding catch resignals
(qa (qa335-order '(condition-case nil (condition-case err (handler-bind ((error (lambda (_) (push 'inner qa335-seen)))) (error "x")) (error (signal (car err) (cdr err)))) (error 'caught)))
    '(caught (inner)))

;; outer observer remains hidden across two catching levels
(qa (qa335-order '(handler-bind ((error (lambda (_) (push 'wrong qa335-seen)))) (condition-case nil (condition-case err (error "x") (error (signal 'file-error '("again")))) (error 'caught))))
    '(caught nil))

;; error in an observer supersedes original signal
(qa (qa335-order '(condition-case err (handler-bind ((error (lambda (_) (push 'observer qa335-seen) (signal 'file-error '("callback"))))) (error "x")) (error (car err))))
    '(file-error (observer)))

;; bytecompiled outer handler cannot steal inner catch
(qa (qa335-order '(catch 'out (handler-bind ((error (lambda (_) (push 'outer qa335-seen) (throw 'out 'stolen)))) (condition-case nil (error "x") (error 'caught)))) t)
    '(caught nil))

;; bytecompiled resignal is offered once
(qa (qa335-order '(condition-case nil (handler-bind ((error (lambda (_) (push 'observer qa335-seen)))) (condition-case err (error "x") (error (signal (car err) (cdr err))))) (error 'caught)) t)
    '(caught (observer)))

;; Dynamic handler state survives collection and a cooperative yield.
(qa (qa335-order '(condition-case nil
                      (handler-bind (((error file-error) (lambda (_) (push 'inner qa335-seen))))
                        (garbage-collect)
                        (error "x"))
                    (error 'caught)))
    '(caught (inner)))
(qa (qa335-order '(handler-bind ((error (lambda (_) (push 'outer qa335-seen))))
                   (condition-case nil (progn (garbage-collect) (error "x"))
                     (error 'caught))))
    '(caught nil))
(qa (let ((worker (make-thread
                  (lambda ()
                    (qa335-order '(condition-case nil
                                      (handler-bind ((error (lambda (_) (push 'thread qa335-seen))))
                                        (sleep-for 0.001) (error "x"))
                                    (error 'caught)))))))
      (thread-join worker))
    '(caught (thread)))

;; Compiled normal completion and throw must remove VM case scopes.
(qa (qa335-order '(condition-case nil
                      (handler-bind ((error (lambda (_) (push 'observer qa335-seen))))
                        (condition-case nil 'ok (error 'bad))
                        (error "x"))
                    (error 'caught)) t)
    '(caught (observer)))
(qa (qa335-order '(condition-case nil
                      (handler-bind ((error (lambda (_) (push 'observer qa335-seen))))
                        (catch 'out
                          (condition-case nil (throw 'out 'ok) (error 'bad)))
                        (error "x"))
                    (error 'caught)) t)
    '(caught (observer)))
(qa (qa335-order '(condition-case nil
                      (handler-bind ((error (lambda (_) (push 'observer qa335-seen))))
                        (condition-case nil (error "x") (file-error 'bad)))
                    (error 'caught)) t)
    '(caught (observer)))

(princ (qa--done))
