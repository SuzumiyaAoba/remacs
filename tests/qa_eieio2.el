;; eieio.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - defclass + make-instance + oref/oset work.
;;   - cl-defgeneric/cl-defmethod dispatch on class.
;;   - eieio-object-p, class-of, slot-boundp, slot-exists-p.
;;   - slot-exists-p returns the slot INDEX (1-based).
;;   - child-of-class-p is the subclass predicate (subclassp NOT a fn).
;;   - eieio-class-children returns the direct subclasses.
;;   - same-class-p compares CLASSES (find-class result), not objects.
;;   - eieio-oref-default returns the raw default value.

(require 'eieio)
(require 'cl-generic)

;; == surface ==
(qa (fboundp 'eieio-defclass-internal) t)
(qa (fboundp 'eieio-object-p) t)
(qa (fboundp 'eieio-oref) t)
(qa (fboundp 'eieio-oset) t)
(qa (fboundp 'eieio-oref-default) t)
(qa (fboundp 'oref) t)
(qa (fboundp 'oset) t)
(qa (fboundp 'oref-default) t)
(qa (fboundp 'class-of) t)
(qa (fboundp 'class-name) t)
(qa (fboundp 'class-direct-superclasses) t)
(qa (fboundp 'make-instance) t)
(qa (fboundp 'defclass) t)
(qa (fboundp 'slot-boundp) t)
(qa (fboundp 'slot-exists-p) t)
(qa (fboundp 'slot-value) t)
(qa (fboundp 'with-slots) t)
(qa (fboundp 'find-class) t)
(qa (fboundp 'eieio-class-children) t)
(qa (fboundp 'same-class-p) t)
(qa (fboundp 'child-of-class-p) t)
;; NOT fns
(qa (fboundp 'eieio-class-p) nil)
(qa (fboundp 'eieio-named-p) nil)
(qa (fboundp 'eieio-slot-name-index) nil)
(qa (fboundp 'subclassp) nil)
(qa (fboundp 'eieio--class-parent) nil)

;; == defclass + make-instance ==
(defclass pt ()
  ((x :initarg :x :initform 0)
   (y :initarg :y :initform 0)))
(let ((o (make-instance 'pt :x 3 :y 4)))
  (qa (oref o x) 3)
  (qa (oref o y) 4)
  (oset o x 99)
  (qa (oref o x) 99)
  (qa (slot-boundp o 'x) t)
  (qa (class-of o) 'pt)
  (qa (eieio-object-p o) t))
(let ((o (make-instance 'pt)))
  (qa (oref o x) 0)
  (qa (oref o y) 0))

;; == cl-defgeneric + cl-defmethod dispatch ==
(cl-defgeneric area (o) "doc")
(cl-defmethod area ((o pt)) (* (oref o x) (oref o y)))
(let ((o (make-instance 'pt :x 3 :y 4)))
  (qa (area o) 12))

;; == subclass inherits + extends slots ==
(defclass pt3d (pt)
  ((z :initarg :z :initform 0)))
(let ((o (make-instance 'pt3d :x 1 :y 2 :z 3)))
  (qa (oref o x) 1)
  (qa (oref o z) 3)
  (qa (child-of-class-p 'pt3d 'pt) t)
  (qa (child-of-class-p 'pt 'pt3d) nil))

;; == same-class-p — (OBJ CLASS) — exact-class check ==
(let ((o1 (make-instance 'pt))
      (o3 (make-instance 'pt3d)))
  (qa (same-class-p o1 'pt) t)
  (qa (same-class-p o1 'pt3d) nil)
  (qa (same-class-p o3 'pt3d) t)
  ;; pt3d IS a subclass of pt but same-class-p tests exact class
  (qa (same-class-p o3 'pt) nil))

;; == eieio-class-children — direct subclasses ==
(let ((kids (eieio-class-children 'pt)))
  (qa (memq 'pt3d kids) '(pt3d)))

;; == class-direct-superclasses — returns eieio--class OBJECTS ==
(let ((supers (class-direct-superclasses 'pt3d)))
  (qa (listp supers) t)
  (qa (class-name (car supers)) 'pt))

;; == slot-exists-p — returns 1-based index ==
(let ((o (make-instance 'pt)))
  (qa (slot-exists-p o 'x) 1)
  (qa (slot-exists-p o 'y) 2)
  (qa (slot-exists-p o 'nosuch) nil))

;; == with-slots ==
(let ((o (make-instance 'pt :x 10 :y 20)))
  (with-slots (x y) o
    (qa (+ x y) 30)))

;; == find-class returns a class object (not eieio-object-p) ==
(let ((cls (find-class 'pt)))
  (qa (eieio-object-p cls) nil)
  (qa (class-name cls) 'pt))

;; == slot-value ==
(let ((o (make-instance 'pt :x 5)))
  (qa (slot-value o 'x) 5))

;; == oset on subclass ==
(let ((o (make-instance 'pt3d)))
  (oset o z 99)
  (qa (oref o z) 99))

;; == eieio-oref / -oset synonyms ==
(let ((o (make-instance 'pt :x 1)))
  (eieio-oset o 'x 42)
  (qa (eieio-oref o 'x) 42))

;; == eieio-oref-default — raw default value ==
(let ((d (eieio-oref-default 'pt 'x)))
  (qa d 0))

;; == nil/non-objects aren't eieio objects ==
(qa (eieio-object-p nil) nil)
(qa (eieio-object-p '(a b)) nil)
(qa (eieio-object-p 42) nil)

;; == named slots ==
(defclass named ()
  ((name :initarg :name :initform "unnamed")))
(let ((o (make-instance 'named)))
  (qa (oref o name) "unnamed"))
(let ((o (make-instance 'named :name "zed")))
  (qa (oref o name) "zed"))

;; == slot-boundp on unset slot ==
(defclass unbound ()
  ((s :initarg :s)))
(let ((o (make-instance 'unbound)))
  (qa (slot-boundp o 's) nil))

;; == pt3d is eieio-object-p too ==
(let ((o (make-instance 'pt3d)))
  (qa (eieio-object-p o) t)
  (qa (class-of o) 'pt3d))

(princ (qa--done))
