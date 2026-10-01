;; eieio — classes, slots, accessors, CLOS-style generics and predicates.
;; defmethod/call-next-method/next-method-p live in eieio-compat (obsolete
;; names); cl-next-method-p emits its obsolete-macro warning via `message'
;; during eager macroexpansion at load time, so inhibit-message is setq'd t
;; for the whole load and restored to nil at the bottom.

;; remacs: `(require 'eieio)' used to never terminate (loader recursed
;; through a mis-parsed cl--loop-expand); both loads work now.

(setq warning-suppress-types '((obsolete))
      inhibit-message t)
(require 'eieio)
(require 'eieio-base)
(require 'eieio-compat) ;; for defmethod/call-next-method/next-method-p
(require 'cl-lib)
(progn

;; == defclass / make-instance ==

(defvar qaee--log nil) ;; dynamic var: methods push here for ordering tests

(defclass qaee-shape ()
  ((name :initarg :name :initform 'anon)
   (x :initarg :x :initform 0 :accessor qaee-shape-x)
   (count :initform 0 :allocation :class)))

(defclass qaee-circle (qaee-shape)
  ((radius :initarg :radius :initform 1)))

;; Abstract classes still instantiate in EIEIO (:abstract is advisory).
(defclass qaee-abstract () () :abstract t)

(qa (eieio-object-p (make-instance 'qaee-shape)) t)
(qa (oref (make-instance 'qaee-shape) name) 'anon)
(qa (oref (make-instance 'qaee-shape :x 7) x) 7)
(qa (oref (make-instance 'qaee-circle) name) 'anon)     ;; inherited initform
(qa (oref (make-instance 'qaee-circle :radius 4) radius) 4)
(qa (oref (make-instance 'qaee-circle :x 2 :radius 4) x) 2)
(qa (eieio-object-p (make-instance 'qaee-abstract)) t)  ;; abstract instantiates
(qa (type-of (make-instance 'qaee-shape)) 'qaee-shape)
(qa-err (make-instance 'qaee-shape :nope 1) 'invalid-slot-name)
;; find-class returns the class object; eieio-class-name projects it.
(qa (eieio-class-name (find-class 'qaee-circle)) 'qaee-circle)
(qa (find-class 'qaee-nope nil) nil)
(qa-err-data (find-class 'qaee-nope t) 'wrong-type-argument '(class-p qaee-nope))
;; make-instance requires a class name symbol, not a class object.
(qa-err (make-instance (find-class 'qaee-shape)) 'cl-no-applicable-method)

;; == slot access ==

(let ((o (make-instance 'qaee-shape :x 3)))
  (qa (oref o x) 3)                    ;; oref slot arg is UNQUOTED
  (qa (progn (oset o x 9) (oref o x)) 9)
  (qa (qaee-shape-x o) 9)              ;; :accessor
  (qa (progn (setf (qaee-shape-x o) 11) (qaee-shape-x o)) 11)
  (qa (slot-value o 'x) 11)            ;; function: slot is quoted
  (qa (progn (setf (slot-value o 'x) 8) (oref o x)) 8)
  (qa (eieio-oref o 'x) 8)             ;; eieio- functions take quoted slot
  (qa (progn (eieio-oset o 'x 42) (eieio-oref o 'x)) 42)
  (qa (slot-boundp o 'x) t)
  (qa (slot-exists-p o 'x) 2)          ;; 1-based slot index, not boolean
  (qa (slot-exists-p o 'name) 1)
  (qa (slot-exists-p o 'zzz) nil)
  (qa (slot-exists-p 'qaee-shape 'x) 2) ;; accepts class name too
  (qa-err (oref o zzz) 'invalid-slot-name)
  (qa-err (oset o zzz 1) 'invalid-slot-name)
  (qa-err (slot-boundp o 'zzz) 'invalid-slot-name)
  (qa-err (slot-value o 'zzz) 'invalid-slot-name)
  (qa (oref-default o x) 0)            ;; class default, not instance value
  (qa (oref-default 'qaee-shape x) 0)
  (qa (eieio-oref-default 'qaee-shape 'x) 0)
  ;; unbound slot machinery
  (slot-makeunbound o 'x)
  (qa (slot-boundp o 'x) nil)
  (qa-err (oref o x) 'unbound-slot)
  (qa-prin1 o "#s(qaee-shape anon eieio--unbound)")) ;; class slot not printed

;; slot-makunbound/eieio-class-p/class-constructor do NOT exist in GNU 31.1
;; (the correct names are slot-makeunbound / eieio--class-p).
(qa (fboundp 'slot-makunbound) nil)
(qa (fboundp 'eieio-class-p) nil)

;; :allocation :class slots are shared and live outside the instance record
(defclass qaee-cslot () ((cs :initform 3 :allocation :class)))
(let ((a (make-instance 'qaee-cslot))
      (b (make-instance 'qaee-cslot)))
  (qa (oref a cs) 3)
  (qa (slot-boundp a 'cs) t)
  (oset a cs 4)
  (qa (oref b cs) 4)                          ;; shared between instances
  (qa (oref-default 'qaee-cslot cs) 4))       ;; stored in the class default

;; :reader/:writer accessors
(defclass qaee-acc ()
  ((ar :initarg :ar :reader qaee-acc-r :writer qaee-acc-w :initform 7)))
(let ((o (make-instance 'qaee-acc)))
  (qa (qaee-acc-r o) 7)
  (qaee-acc-w o 9)
  (qa (qaee-acc-r o) 9))

;; :type slot: violating the declared type signals invalid-slot-type
(defclass qaee-typed () ((tt :initarg :tt :type integer :initform 0)))
(qa-err (make-instance 'qaee-typed :tt "s") 'invalid-slot-type)

;; initforms re-evaluate per instantiation
(defvar qaee--counter 0)
(defclass qaee-icnt () ((v :initform (setq qaee--counter (1+ qaee--counter)))))
(let ((qaee--counter 0))
  (let ((o1 (make-instance 'qaee-icnt))
        (o2 (make-instance 'qaee-icnt)))
    (qa (list (oref o1 v) (oref o2 v) qaee--counter) '(1 2 2))))

;; oset-default mutates the class default for new instances
(defclass qaee-deflt () ((d :initarg :d :initform 1)))
(let ((o (make-instance 'qaee-deflt)))
  (qa (oref-default 'qaee-deflt d) 1)
  (oset-default 'qaee-deflt d 77)
  (qa (oref-default 'qaee-deflt d) 77)
  (qa (oref o d) 1)                          ;; existing instance unaffected
  (qa (oref (make-instance 'qaee-deflt) d) 77))

;; == predicates ==

(let ((c (make-instance 'qaee-circle))
      (s (make-instance 'qaee-shape)))
  (qa (eieio-object-p c) t)
  (qa (eieio-object-p 5) nil)
  (qa (eieio-object-p 'qaee-shape) nil)      ;; class symbol is not an object
  (qa (eieio-object-p (find-class 'qaee-shape)) nil) ;; class object neither
  (qa (eieio--class-p (find-class 'qaee-circle)) t)
  (qa (eieio--class-p 'qaee-circle) nil)
  (qa (eieio-object-class c) 'qaee-circle)   ;; returns the NAME symbol
  (qa (object-class c) 'qaee-circle)
  (qa (object-class-name c) 'qaee-circle)
  (qa (eieio-class-name (find-class 'qaee-shape)) 'qaee-shape)
  (qa (child-of-class-p 'qaee-circle 'qaee-shape) t)
  (qa (child-of-class-p 'qaee-shape 'qaee-circle) nil)
  (qa (child-of-class-p 'qaee-shape 'qaee-shape) t)  ;; reflexive
  (qa (child-of-class-p (find-class 'qaee-circle) 'qaee-shape) t)
  ;; first arg must be a class name/object, NOT an instance
  (qa-err (child-of-class-p c 'qaee-shape) 'args-out-of-range)
  (qa (object-of-class-p c 'qaee-shape) t)   ;; transitive
  (qa (object-of-class-p c 'qaee-circle) t)
  (qa (object-of-class-p s 'qaee-circle) nil)
  (qa (same-class-p c 'qaee-circle) t)
  (qa (same-class-p c 'qaee-shape) nil)
  (qa (same-class-p s 'qaee-shape) t)
  ;; same-class-p requires an object as first arg
  (qa-err (same-class-p 'qaee-circle 'qaee-circle) 'wrong-type-argument)
  (qa-err (eieio-oref 5 'x) 'wrong-type-argument))

;; multiple inheritance: CPL is depth-first, parents in defclass order
(defclass qaee-mi1 () ((a1 :initform 'i1)))
(defclass qaee-mi2 () ((a2 :initform 'i2)))
(defclass qaee-mi3 (qaee-mi1 qaee-mi2) ())

;; class-precedence-list mixes eieio--class and built-in-class objects;
;; eieio-class-name rejects built-in-class entries -> project conditionally.
(qa (mapcar (lambda (c) (and (eieio--class-p c) (eieio-class-name c)))
            (class-precedence-list 'qaee-circle))
    '(qaee-circle qaee-shape eieio-default-superclass nil nil nil))
(qa (mapcar (lambda (c) (and (eieio--class-p c) (eieio-class-name c)))
            (class-precedence-list 'qaee-mi3))
    '(qaee-mi3 qaee-mi1 qaee-mi2 eieio-default-superclass nil nil nil))
(qa (eieio--class-name (nth 3 (class-precedence-list 'qaee-circle))) 'record)
(qa-err (eieio-class-name (nth 3 (class-precedence-list 'qaee-circle)))
        'wrong-type-argument)
(qa (mapcar #'eieio-class-name (eieio-class-parents 'qaee-circle))
    '(qaee-shape))
(qa (mapcar #'eieio-class-name (eieio-class-children 'qaee-shape))
    '(qaee-circle))
(qa (mapcar #'eieio-class-name (class-direct-superclasses 'qaee-circle))
    '(qaee-shape))
(qa (mapcar #'eieio-class-name (class-direct-subclasses 'qaee-shape))
    '(qaee-circle))

;; == cl-defgeneric / cl-defmethod dispatch ==

(cl-defgeneric qaee-area (o) "Area of O.")
(cl-defmethod qaee-area ((o qaee-circle)) (* 3 (oref o radius)))
(qa (qaee-area (make-instance 'qaee-circle :radius 2)) 6)
(qa-err (qaee-area 5) 'cl-no-applicable-method)
(qa-err-data (qaee-area 5) 'cl-no-applicable-method '(qaee-area 5))
(qa (and (cl-generic-p #'qaee-area) t) t) ;; returns the cl--generic object
(qa (not (cl-generic-p #'car)) t)

;; specializers: t (unspecialized), built-in types, (eql …), (head …)
(cl-defgeneric qaee-kind (x))
(cl-defmethod qaee-kind (x) (list 'default x))
(cl-defmethod qaee-kind ((x string)) (list 'str x))
(cl-defmethod qaee-kind ((x integer)) 'int)
(cl-defmethod qaee-kind ((x symbol)) 'sym)
(cl-defmethod qaee-kind ((x (eql 7))) 'seven)
(cl-defmethod qaee-kind ((x (head qaee-tag))) (list 'head (cdr x)))
(qa (qaee-kind 3) 'int)
(qa (qaee-kind 7) 'seven)                ;; eql beats integer
(qa (qaee-kind 'a) 'sym)
(qa (qaee-kind "hi") '(str "hi"))
(qa (qaee-kind '(qaee-tag a b)) '(head (a b)))
(qa (qaee-kind '(other a)) '(default (other a)))
(qa (qaee-kind 1.5) '(default 1.5))

;; == qualifier ordering: :before most->least, primary, :after least->most ==

(cl-defgeneric qaee-emit (o))
(cl-defmethod qaee-emit ((o qaee-shape)) (push 'prim-shape qaee--log) 'shape-v)
(cl-defmethod qaee-emit ((o qaee-circle))
  (push 'prim-circle qaee--log)
  (cl-call-next-method))
(cl-defmethod qaee-emit :before ((o qaee-shape)) (push 'b-shape qaee--log))
(cl-defmethod qaee-emit :before ((o qaee-circle)) (push 'b-circle qaee--log))
(cl-defmethod qaee-emit :after ((o qaee-shape)) (push 'a-shape qaee--log))
(cl-defmethod qaee-emit :after ((o qaee-circle)) (push 'a-circle qaee--log))
(let ((qaee--log nil))
  (qa (qaee-emit (make-instance 'qaee-circle)) 'shape-v)
  (qa (reverse qaee--log)
      '(b-circle b-shape prim-circle prim-shape a-shape a-circle)))

;; :around wraps the rest; return value comes from :around
(cl-defgeneric qaee-m2 (o))
(cl-defmethod qaee-m2 ((o qaee-shape)) (push 'prim qaee--log) 'primval)
(cl-defmethod qaee-m2 :around ((o qaee-shape))
  (push 'around qaee--log)
  (list 'wrapped (cl-call-next-method)))
(cl-defmethod qaee-m2 :before ((o qaee-shape)) (push 'b qaee--log))
(cl-defmethod qaee-m2 :after ((o qaee-shape)) (push 'a qaee--log))
(let ((qaee--log nil))
  (qa (qaee-m2 (make-instance 'qaee-shape)) '(wrapped primval))
  (qa (reverse qaee--log) '(around b prim a)))

;; cl-next-method-p / cl-call-next-method inside methods
(cl-defgeneric qaee-np (o))
(cl-defmethod qaee-np ((o qaee-shape)) (cl-next-method-p))
(cl-defmethod qaee-np ((o qaee-circle))
  (list (cl-next-method-p) (cl-call-next-method)))
(qa (qaee-np (make-instance 'qaee-circle)) '(t nil))
(qa (qaee-np (make-instance 'qaee-shape)) nil)

(cl-defgeneric qaee-cnm2 (o))
(cl-defmethod qaee-cnm2 ((o qaee-shape)) (cl-call-next-method))
(qa-err (qaee-cnm2 (make-instance 'qaee-shape)) 'cl-no-next-method)

;; old eieio-compat names: defmethod / call-next-method / next-method-p
(cl-defgeneric qaee-oldm (o))
(defmethod qaee-oldm ((o qaee-shape)) 'old-shape)
(qa (qaee-oldm (make-instance 'qaee-shape)) 'old-shape)

(cl-defgeneric qaee-cnm (o))
(defmethod qaee-cnm ((o qaee-circle)) (call-next-method))
(defmethod qaee-cnm ((o qaee-shape)) 'base)
(qa (qaee-cnm (make-instance 'qaee-circle)) 'base)

(cl-defgeneric qaee-nmp (o))
(defmethod qaee-nmp ((o qaee-shape)) (next-method-p))
(qa (qaee-nmp (make-instance 'qaee-shape)) nil)

;; defmethod without cl-defgeneric creates an eieio generic on the fly
(defmethod qaee-oldonly ((o qaee-shape)) 'oo)
(qa (qaee-oldonly (make-instance 'qaee-shape)) 'oo)
(qa-err (qaee-oldonly (make-instance 'qaee-cslot)) 'cl-no-applicable-method)

;; == initialize-instance hook / with-slots ==

(cl-defmethod initialize-instance :after ((o qaee-shape) &rest _)
  (push 'init qaee--log))
(let ((qaee--log nil))
  (make-instance 'qaee-shape :x 1)
  (qa qaee--log '(init)))

(cl-defgeneric qaee-describe (o))
(cl-defmethod qaee-describe ((o qaee-circle))
  (with-slots (x radius) o (list x radius)))
(qa (qaee-describe (make-instance 'qaee-circle :x 2 :radius 6)) '(2 6))

;; == clone ==

(let ((o (make-instance 'qaee-circle :x 4 :radius 9)))
  (let ((c (clone o)))
    (qa (list (oref c x) (oref c radius)) '(4 9))
    (qa (not (eq c o)) t))                 ;; clone is a distinct object
  ;; clone takes initargs, not bare slot names
  (let ((c2 (clone o :radius 5)))
    (qa (list (oref c2 x) (oref c2 radius)) '(4 5)))
  (slot-makeunbound o 'x)
  (let ((c3 (clone o)))
    (qa (slot-boundp c3 'x) nil)))          ;; unboundness is cloned

;; == object naming ==

(defclass qaee-named (eieio-named) ())
(let ((o (make-instance 'qaee-named)))
  (oset o object-name 'myn)
  ;; eieio-named's method returns the raw slot value (a symbol here)
  (qa (oref o object-name) 'myn)
  (qa (object-name-string o) 'myn)
  (qa (eieio-object-name-string o) 'myn)
  (qa-prin1 o "#s(qaee-named myn)"))
;; anonymous objects get a non-deterministic name; pin the type only
(qa (stringp (object-name-string (make-instance 'qaee-shape))) t)

(setq inhibit-message nil)
) ;; end (unless qa--remacs ...)
(princ (qa--done))
