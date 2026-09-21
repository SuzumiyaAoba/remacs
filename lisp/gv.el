;;; gv.el --- generalized variables, subset of GNU gv.el -*- lexical-binding: t -*-

;; `gv-get' runs at macro-expansion time: DO is a function called with
;; GETTER (a copyable form returning PLACE's value) and SETTER (a
;; function mapping a value form to a form storing it).
(defun gv-get (place do)
  "Build the code that applies DO to PLACE."
  (cond
   ((symbolp place)
    (funcall do place (lambda (v) (list 'setq place v))))
   ((not (consp place)) (signal 'gv-invalid-place (list place)))
   (t (let* ((head (car place))
             (gf (and (symbolp head) (get head 'gv-expander))))
        (if gf
            (apply gf do (cdr place))
          ;; Unknown head: defer to `cl--setf-pair', which knows the
          ;; built-in places and falls back to a `(setf HEAD)' function.
          (funcall do place
                   (lambda (v) (cl--setf-pair place v))))))))

(defmacro gv-letplace (vars place &rest body)
  "Bind GETTER/SETTER for PLACE, then expand BODY to a form."
  (declare (indent 2))
  (list 'gv-get place (cons 'lambda (cons vars body))))

(defmacro gv-define-expander (name handler)
  "Attach HANDLER as the gv-expander of NAME."
  (list 'function-put (list 'quote name) ''gv-expander handler))

(defmacro gv-define-setter (name arglist &rest body)
  "Register a setter expander for (NAME . ARGS); ARGLIST is (STORE . ARGS)."
  (let ((store (car arglist))
        (args (cdr arglist)))
    `(gv-define-expander ,name
       (lambda (do ,@args)
         (funcall do (list ',name ,@args)
                  (lambda (,store) ,@body))))))

(defmacro gv-define-simple-setter (name setter &optional fix-return)
  "Register a simple setter: (setf (NAME ARGS...) VAL) calls SETTER."
  `(gv-define-setter ,name (val &rest args)
     (cons ',setter (cons val args))))

(defmacro gv-ref (place)
  "Return a reference to PLACE (a cons of getter and setter functions)."
  (gv-letplace (getter setter) place
    (list 'cons (list 'lambda nil getter)
          (list 'lambda '(gv--val) (funcall setter 'gv--val)))))

(defsubst gv-deref (ref)
  "Dereference REF, returning the referenced value."
  (funcall (car ref)))

(gv-define-setter gv-deref (v ref)
  (list 'funcall (list 'cdr ref) v))

(provide 'gv)
