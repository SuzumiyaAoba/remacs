;;; -*- lexical-binding: t; -*-
;; Issue #182 caller probes. Load tests/qa_framework.el before this file.
;; Independently exercises propagation, short-circuiting, partial mutation,
;; and bytecode handler dispatch. Expected values verified against GNU Emacs 31.1.
(require 'seq)
(require 'cl-lib)

(defun qa182-seq-deep (&optional depth leaf)
  (let ((x (or leaf 1)))
    (dotimes (_ (or depth 205)) (setq x (list x)))
    x))
(defvar qa182-seq-a (qa182-seq-deep))
(defvar qa182-seq-b (qa182-seq-deep))

(qa-err-data (member qa182-seq-a (list qa182-seq-b))
    'error '("Stack overflow in equal"))
(qa-err-data (assoc qa182-seq-a (list (cons qa182-seq-b 'value)))
    'error '("Stack overflow in equal"))
(qa-err-data (rassoc qa182-seq-a (list (cons 'key qa182-seq-b)))
    'error '("Stack overflow in equal"))
(qa-err-data (assoc-default qa182-seq-a (list (cons qa182-seq-b 'value)))
    'error '("Stack overflow in equal"))
(qa-err-data (delete qa182-seq-a (list qa182-seq-b))
    'error '("Stack overflow in equal"))
(qa-err-data (delete qa182-seq-a (vector qa182-seq-b))
    'error '("Stack overflow in equal"))
(qa-err-data (remove qa182-seq-a (list qa182-seq-b))
    'error '("Stack overflow in equal"))
(qa-err-data (remove qa182-seq-a (vector qa182-seq-b))
    'error '("Stack overflow in equal"))
(qa-err-data (lax-plist-get (list qa182-seq-b 'value) qa182-seq-a)
    'error '("Stack overflow in equal"))
(qa-err-data (plist-get (list qa182-seq-b 'value) qa182-seq-a #'equal)
    'error '("Stack overflow in equal"))
(qa-err-data (seq-contains-p (list qa182-seq-b) qa182-seq-a)
    'error '("Stack overflow in equal"))
(qa-err-data (seq-position (vector qa182-seq-b) qa182-seq-a)
    'error '("Stack overflow in equal"))
(qa-err-data (seq-uniq (list qa182-seq-a qa182-seq-b))
    'error '("Stack overflow in equal"))
(qa-err-data (equal-including-properties qa182-seq-a qa182-seq-b)
    'error '("Stack overflow in equal"))
(qa-err-data
    (equal-including-properties (propertize "x" 'deep qa182-seq-a)
                                (propertize "x" 'deep qa182-seq-b))
    'error '("Stack overflow in equal"))

;; Identity matches stop searching before a later overflowing comparison.
(qa (let ((xs (list qa182-seq-a qa182-seq-b)))
      (eq (member qa182-seq-a xs) xs)) t)
(qa (let ((pair (cons qa182-seq-a 'first)))
      (eq (assoc qa182-seq-a (list pair (cons qa182-seq-b 'second))) pair)) t)
(qa (let ((pair (cons 'first qa182-seq-a)))
      (eq (rassoc qa182-seq-a (list pair (cons 'second qa182-seq-b))) pair)) t)
(qa (assoc-default qa182-seq-a
                   (list (cons qa182-seq-a 'first) (cons qa182-seq-b 'second)))
    'first)
(qa (seq-contains-p (list qa182-seq-a qa182-seq-b) qa182-seq-a) t)
(qa (seq-position (vector qa182-seq-a qa182-seq-b) qa182-seq-a) 0)
(qa (equal-including-properties (propertize "a" 'deep qa182-seq-a)
                                (propertize "b" 'deep qa182-seq-b)) nil)
(qa (equal-including-properties
     (concat (propertize "x" 'first 1) (propertize "y" 'deep qa182-seq-a))
     (concat (propertize "x" 'first 2) (propertize "y" 'deep qa182-seq-b))) nil)

;; eq/eql and explicit callback paths keep their existing semantics.
(qa (memq qa182-seq-a (list qa182-seq-b)) nil)
(qa (memql qa182-seq-a (list qa182-seq-b)) nil)
(qa (assq qa182-seq-a (list (cons qa182-seq-b 'value))) nil)
(qa (rassq qa182-seq-a (list (cons 'key qa182-seq-b))) nil)
(qa (assoc qa182-seq-a (list (cons qa182-seq-b 'value)) #'eq) nil)
(qa (seq-contains-p (list qa182-seq-b) qa182-seq-a #'eq) nil)
(qa (seq-position (vector qa182-seq-b) qa182-seq-a #'eq) nil)
(qa (length (seq-uniq (list qa182-seq-a qa182-seq-b) #'eq)) 2)
(qa (delete qa182-seq-a "abc") "abc")
(qa (remove qa182-seq-a "abc") "abc")

;; delete must keep already-applied non-head splices when a later comparison
;; errors. remove must leave its original sequence untouched on the same path.
(qa (let ((xs (list 'keep qa182-seq-a qa182-seq-b 'end)))
      (list (condition-case e (delete qa182-seq-a xs) (error e))
            (length xs) (eq (cadr xs) qa182-seq-b)))
    '((error "Stack overflow in equal") 3 t))
(qa (let ((xs (list 'keep qa182-seq-a qa182-seq-b 'end)))
      (list (condition-case e (remove qa182-seq-a xs) (error e))
            (length xs) (eq (cadr xs) qa182-seq-a)))
    '((error "Stack overflow in equal") 4 t))

;; cl-equalp keeps its own number/string/list/vector semantics. Bytecode
;; functions use its equal fallback; nesting them tests fallible recursion.
(defvar qa182-seq-fa (make-byte-code 0 "\300\207" (vector qa182-seq-a) 1))
(defvar qa182-seq-fb (make-byte-code 0 "\300\207" (vector qa182-seq-b) 1))
(qa-err-data (cl-equalp qa182-seq-fa qa182-seq-fb)
    'error '("Stack overflow in equal"))
(qa-err-data (cl-equalp (list qa182-seq-fa) (list qa182-seq-fb))
    'error '("Stack overflow in equal"))
(qa-err-data (cl-equalp (vector qa182-seq-fa) (vector qa182-seq-fb))
    'error '("Stack overflow in equal"))
(qa (cl-equalp '(1 "AbC") '(1.0 "abc")) t)

;; Direct BEqual, including a handler inside the bytecode function. Using
;; r! in opcode 154 is essential: ordinary ? would skip dispatch handlers.
(qa-err-data (funcall (make-byte-code 0 "\300\301\232\207"
                                     (vector qa182-seq-a qa182-seq-b) 2))
    'error '("Stack overflow in equal"))
(qa (funcall (make-byte-code 0 "\300\061\011\000\301\302\232\060\207\207"
                            (vector '(error) qa182-seq-a qa182-seq-b) 2))
    '(error "Stack overflow in equal"))
(qa (funcall (make-byte-code 0 "\300\301\232\207"
                            (vector '(1) '(1)) 2)) t)

;; String-property values have a fresh equal depth budget, not one reduced
;; by the surrounding plist. Property names compare by eq, in left-hand
;; plist order; the right-hand order is irrelevant.
(defun qa182-seq-string (plist)
  (let ((s (copy-sequence "x")))
    (set-text-properties 0 1 plist s)
    s))
(qa (equal-including-properties
     (qa182-seq-string (list 'deep (qa182-seq-deep 199)))
     (qa182-seq-string (list 'deep (qa182-seq-deep 199)))) t)
(qa (equal-including-properties
     (qa182-seq-string (list 'deep (qa182-seq-deep 200)))
     (qa182-seq-string (list 'deep (qa182-seq-deep 200)))) t)
(qa (equal-including-properties
     (qa182-seq-string (list 'deep (qa182-seq-deep 199 1)))
     (qa182-seq-string (list 'deep (qa182-seq-deep 199 2)))) nil)
(qa (equal-including-properties
     (qa182-seq-string (list 'deep (qa182-seq-deep 200 1)))
     (qa182-seq-string (list 'deep (qa182-seq-deep 200 2)))) nil)
(qa-err-data
    (equal-including-properties
     (qa182-seq-string (list 'deep (qa182-seq-deep 201)))
     (qa182-seq-string (list 'deep (qa182-seq-deep 201))))
    'error '("Stack overflow in equal"))
(qa-err-data
    (equal-including-properties
     (qa182-seq-string (list 'deep (qa182-seq-deep 201 1)))
     (qa182-seq-string (list 'deep (qa182-seq-deep 201 2))))
    'error '("Stack overflow in equal"))
(qa (equal-including-properties
     (qa182-seq-string '(a 1 b 2)) (qa182-seq-string '(b 2 a 1))) t)
(qa (equal-including-properties
     (qa182-seq-string '(a 1)) (qa182-seq-string '(a 1 b 2))) nil)
(qa (equal-including-properties
     (qa182-seq-string '(a nil)) (qa182-seq-string nil)) nil)
(qa (equal-including-properties
     (qa182-seq-string (list (copy-sequence "name") 1))
     (qa182-seq-string (list (copy-sequence "name") 1))) nil)
(qa (let ((key (copy-sequence "name")))
      (equal-including-properties
       (qa182-seq-string (list key 1)) (qa182-seq-string (list key 1)))) t)
(qa (equal-including-properties
     (qa182-seq-string (list qa182-seq-a 1))
     (qa182-seq-string (list qa182-seq-b 1))) nil)
(qa (equal-including-properties
     (qa182-seq-string (list qa182-seq-a 1))
     (qa182-seq-string (list qa182-seq-a 1))) t)
(qa (equal-including-properties
     (qa182-seq-string (list 'first 1 'deep qa182-seq-a))
     (qa182-seq-string (list 'deep qa182-seq-b 'first 2))) nil)
(qa-err-data
    (equal-including-properties
     (qa182-seq-string (list 'deep qa182-seq-a 'later 1))
     (qa182-seq-string (list 'later 2 'deep qa182-seq-b)))
    'error '("Stack overflow in equal"))
(qa-err-data
    (equal-including-properties
     (qa182-seq-string (list 'deep qa182-seq-a))
     (qa182-seq-string (list 'deep qa182-seq-b 'extra 1)))
    'error '("Stack overflow in equal"))
(qa-err-data
    (equal-including-properties
     (qa182-seq-string (list 'deep qa182-seq-a 'extra 1))
     (qa182-seq-string (list 'deep qa182-seq-b)))
    'error '("Stack overflow in equal"))
;; GH #125: the constructor's plist order does not affect property equality.
(qa (equal-including-properties
     (propertize "abc" 'a 1 'b 2)
     (propertize "abc" 'b 2 'a 1)) t)
(qa (equal-including-properties
     (propertize "abc" 'a 1 'b 2)
     (propertize "abc" 'b 3 'a 1)) nil)
(qa (let ((a (propertize "abc" 'a 1 'b 2))
          (b (propertize "abc" 'b 2 'a 1)))
      (add-text-properties 1 2 '(c 3 d 4) a)
      (add-text-properties 1 2 '(d 4 c 3) b)
      (equal-including-properties a b)) t)
(princ (qa--done)) (terpri)
