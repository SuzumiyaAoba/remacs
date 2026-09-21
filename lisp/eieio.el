;;; eieio.el --- EIEIO subset: defclass/make-instance/slot accessors -*- lexical-binding: nil -*-

;; Compatibility subset of GNU EIEIO.  Instances are records printed as
;; `#s(CLASS SLOTS...)', matching GNU's observable representation.
;; Class metadata lives on the class symbol's `eieio--class' property:
;;   (NAME PARENTS SLOTS INITFORMS INITARGS)
;; where SLOTS is the ordered slot list (inherited first), INITFORMS an
;; alist (SLOT . FORM) evaluated per instance, and INITARGS an alist
;; (INITARG . SLOT).

(define-error 'invalid-slot-name "Invalid slot name" 'error)
(define-error 'unbound-slot "Unbound slot" 'error)

(defvar eieio--unbound 'eieio--unbound
  "Marker value for slots that have no initform.")

(defun eieio--class-def (class)
  "Return the class metadata for CLASS (a symbol or class object)."
  (cond ((symbolp class) (get class 'eieio--class))
        ((and (consp class) (eq (car class) 'eieio--class-def))
         (cdr class))
        (t nil)))

(defun eieio--class-name (class)
  (car (eieio--class-def class)))

(defun eieio--class-parents (class)
  (nth 1 (eieio--class-def class)))

(defun eieio--class-slots (class)
  (nth 2 (eieio--class-def class)))

(defun eieio--class-initforms (class)
  (nth 3 (eieio--class-def class)))

(defun eieio--class-initargs (class)
  (nth 4 (eieio--class-def class)))

(defun eieio--collect-slots (name parents own-slots)
  "Ordered full slot list for class NAME: inherited first, then own."
  (let ((slots nil))
    (dolist (p parents)
      (let ((pd (eieio--class-def p)))
        (dolist (s (nth 2 pd))
          (unless (memq s slots) (push s slots)))))
    (dolist (s own-slots)
      (unless (memq s slots) (push s slots)))
    (nreverse slots)))

(defun eieio--collect-initforms (parents own)
  "Merge initform alists: parents first, own overriding."
  (let ((out nil))
    (dolist (p parents)
      (dolist (c (nth 3 (eieio--class-def p)))
        (unless (assq (car c) out) (push c out))))
    (dolist (c own)
      (setq out (cons c (assq-delete-all (car c) out))))
    (nreverse out)))

(defun eieio--collect-initargs (parents own)
  "Merge initarg alists: parents first, own overriding."
  (let ((out nil))
    (dolist (p parents)
      (dolist (c (nth 4 (eieio--class-def p)))
        (unless (assq (car c) out) (push c out))))
    (dolist (c own)
      (setq out (cons c (assq-delete-all (car c) out))))
    (nreverse out)))

(defmacro defclass (name parents slots &rest options)
  "Define NAME as a class with PARENTS and slot specs SLOTS.
Each slot spec is (NAME [:initarg KEY] [:initform FORM] ...)."
  (let* ((own nil) (initforms nil) (initargs nil))
    (dolist (spec slots)
      (let* ((sname (car spec))
             (plist (cdr spec))
             (initform (plist-member plist :initform))
             (initarg (plist-get plist :initarg)))
        (push sname own)
        (when initform (push (cons sname (cadr initform)) initforms))
        (when initarg (push (cons initarg sname) initargs))))
    (setq own (nreverse own)
          initforms (nreverse initforms)
          initargs (nreverse initargs))
    ;; Evaluate at load/compile time so PARENTS can be a quoted list.
    `(let* ((parents ',parents)
            (slots (eieio--collect-slots ',name parents ',own))
            (initforms (eieio--collect-initforms parents ',initforms))
            (initargs (eieio--collect-initargs parents ',initargs)))
       (put ',name 'eieio--class
            (list ',name parents slots initforms initargs))
       (set ',name (cons 'eieio--class-def
                         (get ',name 'eieio--class)))
       ',name)))

(defun find-class (name &rest _)
  "Return the class named NAME, or nil."
  (and (symbolp name)
       (get name 'eieio--class)
       (symbol-value name)))

(defun eieio--slot-index (class slot)
  "Index of SLOT in CLASS's record layout (0-based, after the tag), or nil."
  (let ((slots (eieio--class-slots class))
        (i 1) (found nil))
    (while (and slots (not found))
      (if (eq (car slots) slot) (setq found i) (setq i (1+ i) slots (cdr slots))))
    found))

(defun eieio-object-p (obj)
  "Non-nil if OBJ is an EIEIO instance."
  (and (recordp obj)
       (> (length obj) 0)
       (symbolp (aref obj 0))
       (get (aref obj 0) 'eieio--class)
       t))

(defun class-of (obj)
  "Class symbol of EIEIO object OBJ, or nil."
  (and (eieio-object-p obj) (aref obj 0)))

(defun object-class (obj)
  "Class name of OBJ (obsolete alias of `class-of')."
  (class-of obj))

(defun eieio-object-name (obj)
  "Printed name string of OBJ, like GNU's `#<CLASS HASH>'."
  (format "#<%s %x>" (class-of obj) (logand (sxhash-eq obj) #xfffffff)))

(defun make-instance (class &rest args)
  "Create an instance of CLASS with :initarg ARGS."
  (let* ((name (if (symbolp class) class (eieio--class-name class)))
         (def (eieio--class-def name)))
    (unless def
      (signal 'cl-no-applicable-method
              (list 'make-instance class)))
    (let* ((slots (nth 2 def))
           (initforms (nth 3 def))
           (initargs (nth 4 def))
           (obj (make-record name (length slots) eieio--unbound)))
      ;; Apply initforms.
      (dolist (s slots)
        (let ((cell (assq s initforms)))
          (when cell
            (aset obj (eieio--slot-index name s) (eval (cdr cell))))))
      ;; Apply initargs.
      (let ((as args))
        (while as
          (let* ((k (car as)) (v (cadr as))
                 (slot (cdr (assq k initargs))))
            (unless slot
              (signal 'invalid-slot-name (list name k)))
            (aset obj (eieio--slot-index name slot) v)
            (setq as (cddr as)))))
      obj)))

(defun eieio-oref (obj slot)
  "Return the value of OBJ's SLOT; signals `unbound-slot' if unset."
  (let* ((class (class-of obj))
         (i (and class (eieio--slot-index class slot))))
    (unless i
      (signal 'wrong-type-argument (list 'symbol slot 'slot)))
    (let ((v (aref obj i)))
      (if (eq v eieio--unbound)
          (signal 'unbound-slot (list class (list obj slot 'oref)))
        v))))

(defun eieio-oset (obj slot value)
  "Set OBJ's SLOT to VALUE."
  (let* ((class (class-of obj))
         (i (and class (eieio--slot-index class slot))))
    (unless i
      (signal 'wrong-type-argument (list 'symbol slot 'slot)))
    (aset obj i value)))

(defmacro oref (obj slot)
  "Return OBJ's SLOT (SLOT is a symbol literal)."
  `(eieio-oref ,obj ',slot))

(defmacro oset (obj slot value)
  "Set OBJ's SLOT to VALUE."
  `(eieio-oset ,obj ',slot ,value))

(defun slot-value (obj slot)
  "Return OBJ's SLOT (SLOT quoted).  Like `eieio-oref'."
  (eieio-oref obj slot))

;; `(setf (slot-value o s) v)' and `(setf (oref o s) v)' write through.
(fset (intern "(setf slot-value)") (lambda (v o s) (eieio-oset o s v)))
(fset (intern "(setf oref)") (lambda (v o s) (eieio-oset o s v)))
(fset (intern "(setf eieio-oref)") (lambda (v o s) (eieio-oset o s v)))

(defun set-slot-value (obj slot value)
  (eieio-oset obj slot value))

(defun slot-boundp (obj slot)
  "Non-nil if OBJ's SLOT is set."
  (let* ((class (class-of obj))
         (i (and class (eieio--slot-index class slot))))
    (and i (not (eq (aref obj i) eieio--unbound)))))

(defun slot-exists-p (obj-or-class slot)
  "Non-nil if SLOT is defined for OBJ-OR-CLASS."
  (let ((class (if (eieio-object-p obj-or-class)
                   (class-of obj-or-class)
                 obj-or-class)))
    (eieio--slot-index class slot)))

(defmacro with-slots (spec-list object &rest body)
  "Bind slot names in SPEC-LIST to OBJECT's slots while running BODY.
Matches GNU: `let*' binds OBJECT to a `object' temp, then
`cl-symbol-macrolet' maps each slot name to a `slot-value' form so
`setq' writes through."
  `(let* ((object ,object))
     (cl-symbol-macrolet
         ,(mapcar (lambda (entry)
                    (let ((var (if (consp entry) (car entry) entry))
                          (slot (if (consp entry) (cadr entry) entry)))
                      `(,var (slot-value object ',slot))))
                  spec-list)
       ,@body)))

(defmacro oref-default (class slot)
  "Default (initform) value of CLASS's SLOT."
  `(eieio-oref-default ,class ',slot))

(defun eieio-oref-default (class slot)
  "Default value of CLASS's SLOT (evaluated initform)."
  (let* ((def (eieio--class-def class))
         (cell (assq slot (nth 3 def))))
    (if cell (eval (cdr cell)) eieio--unbound)))

(defmacro oset-default (class slot value)
  `(eieio-oset-default ,class ',slot ,value))

(defun eieio-oset-default (class slot value)
  (let ((def (eieio--class-def class)))
    (put (car def) 'eieio--class
         (list (car def) (nth 1 def) (nth 2 def)
               (cons (cons slot value)
                     (assq-delete-all slot (nth 3 def)))
               (nth 4 def))))
  value)

(defun object-of-class-p (obj class)
  "Non-nil if OBJ is an instance of CLASS or a subclass."
  (let ((c (class-of obj)))
    (and c (child-of-class-p c class))))

(defun child-of-class-p (child parent)
  "Non-nil if CHILD class descends from PARENT."
  (or (eq child parent)
      (let ((todo (list child)) (found nil))
        (while todo
          (let* ((c (pop todo))
                 (ps (eieio--class-parents c)))
            (if (memq parent ps)
                (setq found t todo nil)
              (setq todo (append ps todo)))))
        found)))

(defun same-class-p (obj class)
  "Non-nil if OBJ's class is exactly CLASS."
  (eq (class-of obj) class))

;; cl-generic integration: a method specializer that names an EIEIO
;; class dispatches on the object's class.
(defun eieio--class-p (sym)
  (and (symbolp sym) (get sym 'eieio--class) t))

(provide 'eieio)
