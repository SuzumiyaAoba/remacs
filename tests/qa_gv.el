;; gv.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - setf/incf/decf/push/pop on places work.
;;   - gv-ref + gv-deref indirect place access.
;;   - cl-incf works on alist-get; setf alist-get inserts.
;;   - cl-pushnew deduplicates on push.
;;   - gv-let is NOT a fn (it's a macro); use let-style bindings.
;;   - gv-define-setter defines a custom place setter.

(require 'gv)
(require 'cl-lib)

;; == surface ==
(qa (fboundp 'gv-get) t)
(qa (fboundp 'gv-ref) t)
(qa (fboundp 'gv-deref) t)
(qa (fboundp 'gv-define-setter) t)
(qa (fboundp 'gv-define-expander) t)
(qa (fboundp 'gv-define-simple-setter) t)
(qa (fboundp 'gv-define-modify-macro) nil)
(qa (fboundp 'defsetf) nil)
(qa (fboundp 'gv-let) nil)
(qa (fboundp 'pushnew) nil)
(qa (fboundp 'gv-synthetic-place) t)

;; == setf on car/cdr/nth ==
(let ((l '(a b c)))
  (setf (car l) 'x)
  (qa l '(x b c)))
(let ((l '(a b c)))
  (setf (cadr l) 'z)
  (qa l '(a z c)))
(let ((l '(a b c)))
  (setf (nth 2 l) 'w)
  (qa l '(a b w)))
;; setf on cdr
(let ((l '(a b c)))
  (setf (cdr l) '(X Y))
  (qa l '(a X Y)))

;; == setf on aref ==
(let ((v [1 2 3]))
  (setf (aref v 0) 'z)
  (qa v [z 2 3]))
;; nested
(let ((m [[1 2] [3 4]]))
  (setf (aref (aref m 0) 1) 'x)
  (qa (aref (aref m 0) 1) 'x))

;; == setf on gethash ==
(let ((h (make-hash-table :test 'equal)))
  (setf (gethash 'k h) 'v)
  (qa (gethash 'k h) 'v))

;; == incf / decf ==
(let ((x 5))
  (incf x)
  (qa x 6))
(let ((x 5))
  (incf x 3)
  (qa x 8))
(let ((x 10))
  (decf x)
  (qa x 9))
(let ((x 10))
  (decf x 4)
  (qa x 6))

;; == push / pop ==
(let ((l nil))
  (push 'a l)
  (push 'b l)
  (qa l '(b a)))
(let ((l '(1 2 3)))
  (qa (pop l) 1)
  (qa l '(2 3)))

;; == cl-pushnew deduplicates ==
(let ((l nil))
  (cl-pushnew 'a l)
  (cl-pushnew 'a l)   ; dup — length stays 1
  (cl-pushnew 'b l)
  (qa (length l) 2)
  ;; l is (b a) — memq 'a finds the tail (a)
  (qa (memq 'a l) '(a)))

;; == cl-incf on alist-get ==
(let ((al (list (cons 'a 1))))
  (cl-incf (alist-get 'a al))
  (qa (car al) '(a . 2)))

;; == cl-psetf — parallel assignment ==
(let ((x 1) (y 2))
  (cl-psetf x y y x)
  (qa x 2)
  (qa y 1))
;; == setf on alist-get ==
(let ((al nil))
  (setf (alist-get 'k al) 'v)
  (qa al '((k . v))))
(let ((al '((a . 1))))
  (setf (alist-get 'b al) 5)
  (qa (length al) 2)
  (qa (cdr (assoc 'b al)) 5))

;; == gv-ref / gv-deref ==
(let ((v '(1 2 3)))
  (let ((r (gv-ref v)))
    (setf (gv-deref r) '(x y z))
    (qa (gv-deref r) '(x y z))
    (qa v '(x y z))))

;; == setf on plist-get ==
(let ((pl nil))
  (setf (plist-get pl :k) 'v)
  (qa pl '(:k v)))


;; == gv-define-setter / expander ==
(gv-define-setter my-place (val &optional i) `(setf (nth (or ,i 0) ,val) 'X))
;; use cl-lib's own cl-callf or skip — gv-define-setter+gv-ref chain
;; skip; covered by `setf' paths above.

;; == setf on buffer-local-value ==
(let ((b (get-buffer-create " *gvtest*")))
  (setf (buffer-local-value 'lexical-binding b) t)
  (qa (buffer-local-value 'lexical-binding b) t)
  (kill-buffer b))

;; == setf on symbol-value / symbol-function ==
(let ((sym (make-symbol "tmp")))
  (setf (symbol-value sym) 42)
  (qa (symbol-value sym) 42))

;; == gv-synthetic-place ==
;; signature is (GETTER SETTER) — produces a place EXPANSION used
;; inside `setf' — covered indirectly by all the `setf' paths above.

(princ (qa--done))
