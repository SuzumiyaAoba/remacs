;;; thunk.el --- Lazy form evaluation  -*- lexical-binding: t -*-

;; Ported from GNU Emacs thunk.el (Michael Heerdegen).
;; `thunk-delay' builds a self-caching delayed evaluation; `thunk-let'
;; and `thunk-let*' are like `let'/`let*' but evaluate each binding
;; lazily on first use via symbol macros.

;;; Code:

(require 'cl-lib)

(defmacro thunk-delay (&rest body)
  "Delay the evaluation of BODY."
  (declare (debug (def-body)))
  (cl-assert lexical-binding)
  `(let (forced
         (val (lambda () ,@body)))
     (lambda (&optional check)
       (if check
           forced
         (unless forced
           (setf val (funcall val))
           (setf forced t))
         val))))

(defun thunk-force (delayed)
  "Force the evaluation of DELAYED.
The result is cached and will be returned on subsequent calls
with the same DELAYED argument."
  (funcall delayed))

(defun thunk-evaluated-p (delayed)
  "Return non-nil if DELAYED has been evaluated."
  (funcall delayed t))

(defmacro thunk-let (bindings &rest body)
  "Like `let' but create lazy bindings.

BINDINGS is a list of elements of the form (SYMBOL EXPRESSION).
Any binding EXPRESSION is not evaluated before the variable
SYMBOL is used for the first time when evaluating the BODY.

It is not allowed to set `thunk-let' or `thunk-let*' bound
variables.

Using `thunk-let' and `thunk-let*' requires `lexical-binding'."
  (declare (indent 1) (debug let))
  (setq bindings
        (mapcar
         (lambda (binding)
           (if (and (consp binding)
                    (symbolp (car binding))
                    (consp (cdr binding))
                    (null (cddr binding)))
               binding
             (signal 'error
                     (cons "Bad binding in thunk-let" (list binding)))))
         bindings))
  (setq bindings
        (mapcar (lambda (binding)
                  (list (make-symbol (concat (symbol-name (car binding))
                                             "-thunk"))
                        (car binding)
                        (cadr binding)))
                bindings))
  `(let ,(mapcar (lambda (b) `(,(car b) (thunk-delay ,(nth 2 b))))
                 bindings)
     (cl-symbol-macrolet
         ,(mapcar (lambda (b) `(,(cadr b) (thunk-force ,(car b))))
                  bindings)
       ,@body)))

(defmacro thunk-let* (bindings &rest body)
  "Like `let*' but create lazy bindings.

BINDINGS is a list of elements of the form (SYMBOL EXPRESSION).
Any binding EXPRESSION is not evaluated before the variable
SYMBOL is used for the first time when evaluating the BODY.

It is not allowed to set `thunk-let' or `thunk-let*' bound
variables.

Using `thunk-let' and `thunk-let*' requires `lexical-binding'."
  (declare (indent 1) (debug let))
  ;; Emit the GNU nesting (thunk-let ((b1)) (thunk-let ((b2)) ...))
  ;; explicitly: our `cl-symbol-macrolet' substitutes in the raw form,
  ;; so a nested `thunk-let' macro call would have its binding names
  ;; rewritten before expansion.
  (setq bindings
        (mapcar
         (lambda (binding)
           (if (and (consp binding)
                    (symbolp (car binding))
                    (consp (cdr binding))
                    (null (cddr binding)))
               binding
             (signal 'error
                     (cons "Bad binding in thunk-let" (list binding)))))
         bindings))
  (setq bindings
        (mapcar (lambda (binding)
                  (list (make-symbol (concat (symbol-name (car binding))
                                             "-thunk"))
                        (car binding)
                        (cadr binding)))
                bindings))
  (let ((exp (macroexp-progn body)))
    (dolist (b (reverse bindings))
      (setq exp `(let ((,(car b) (thunk-delay ,(nth 2 b))))
                   (cl-symbol-macrolet ((,(cadr b) (thunk-force ,(car b))))
                     ,exp))))
    exp))

(provide 'thunk)
;;; thunk.el ends here
