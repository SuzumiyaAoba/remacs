;;; -*- lexical-binding: t; -*-
;; `defun' splices declaration values into the registered handler call.
(qa (funcall (byte-compile '(lambda (x) (declare (indent 1)) x)) 5) 5)

(defvar qa-defun-decl-seen nil)
(defun qa-defun-decl-handler (name arglist &rest values)
  (list 'setq 'qa-defun-decl-seen
        (list 'quote (list name arglist values))))
(defvar qa-defun-decl-handler-saw nil)
(defvar qa-defun-decl-action-saw nil)
(defvar qa-defun-decl-loadhist-saw nil)
(defun qa-defun-decl-order-handler (name _arglist)
  (setq qa-defun-decl-handler-saw (fboundp name))
  (list 'setq 'qa-defun-decl-action-saw
        (list 'fboundp (list 'quote name))))
(defun qa-defun-decl-loadhist-handler (name _arglist)
  (list 'setq 'qa-defun-decl-loadhist-saw
        (list 'if
              (list 'member (list 'quote (cons 'defun name)) 'current-load-list)
              t nil)))
(defvar qa-defun-decl-live-sequence nil)
(defvar qa-defun-decl-live-result nil)
(defun qa-defun-decl-live-p-handler (_name _arglist)
  (if (vectorp qa-defun-decl-live-sequence)
      (aset qa-defun-decl-live-sequence 1 '(qa-defun-decl-live-r))
    (setcar (cdr qa-defun-decl-live-sequence) '(qa-defun-decl-live-r)))
  nil)
(defun qa-defun-decl-live-q-handler (_name _arglist)
  (list 'setq 'qa-defun-decl-live-result (list 'quote 'q)))
(defun qa-defun-decl-live-r-handler (_name _arglist)
  (list 'setq 'qa-defun-decl-live-result (list 'quote 'r)))
(defun qa-defun-decl-first-handler (_name _arglist)
  (list 'setq 'qa-defun-decl-seen (list 'quote 'first)))
(defun qa-defun-decl-second-handler (_name _arglist)
  (list 'setq 'qa-defun-decl-seen (list 'quote 'second)))

(qa (let ((defun-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler)))
          (qa-defun-decl-seen nil))
      (eval '(defun qa-defun-decl-proper (x)
               (declare (qa-defun-decl-test first second))
               x))
      qa-defun-decl-seen)
    '(qa-defun-decl-proper (x) (first second)))

(qa (let ((defun-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler)))
          (qa-defun-decl-seen nil))
      (eval '(defun qa-defun-decl-doc-prefix ()
               (:documentation "doc")
               (declare (qa-defun-decl-test first))
               42))
      qa-defun-decl-seen)
    '(qa-defun-decl-doc-prefix nil (first)))

(qa (let ((defun-declarations-alist
           '((qa-defun-decl-first qa-defun-decl-first-handler)
             (qa-defun-decl-second qa-defun-decl-second-handler)))
          (qa-defun-decl-seen nil))
      (eval '(defun qa-defun-decl-duplicate ()
               (declare (qa-defun-decl-first))
               (declare (qa-defun-decl-second))
               42))
      qa-defun-decl-seen)
    'first)

(qa (let ((defun-declarations-alist
           '((nil qa-defun-decl-handler)))
          (qa-defun-decl-seen nil))
      (eval '(defun qa-defun-decl-nil-property (x)
               (declare nil)
               x))
      qa-defun-decl-seen)
    '(qa-defun-decl-nil-property (x) nil))

;; GNU runs declaration handlers during macro expansion, before `defalias',
;; then evaluates the returned actions after installing the definition.
(qa (let ((defun-declarations-alist
           '((qa-defun-decl-order qa-defun-decl-order-handler)))
          (qa-defun-decl-handler-saw nil)
          (qa-defun-decl-action-saw nil))
      (eval '(defun qa-defun-decl-order-target (x)
               (declare (qa-defun-decl-order))
               x))
      (list qa-defun-decl-handler-saw qa-defun-decl-action-saw))
    '(nil t))
(qa (let ((defun-declarations-alist
           '((qa-defun-decl-order qa-defun-decl-loadhist-handler)))
          (qa-defun-decl-loadhist-saw nil))
      (eval '(defun qa-defun-decl-loadhist-target (x)
               (declare (qa-defun-decl-order))
               x))
      qa-defun-decl-loadhist-saw)
    t)
(qa (let ((qa-defun-decl-live-sequence
           (vector '(qa-defun-decl-live-p) '(qa-defun-decl-live-q)))
          (qa-defun-decl-live-result nil)
          (defun-declarations-alist
           '((qa-defun-decl-live-p qa-defun-decl-live-p-handler)
             (qa-defun-decl-live-q qa-defun-decl-live-q-handler)
             (qa-defun-decl-live-r qa-defun-decl-live-r-handler))))
      (eval (list 'defun 'qa-defun-decl-live-vector nil
                  (cons 'declare qa-defun-decl-live-sequence)
                  42))
      qa-defun-decl-live-result)
    'r)
(qa (let ((qa-defun-decl-live-sequence
           (list '(qa-defun-decl-live-p) '(qa-defun-decl-live-q)))
          (qa-defun-decl-live-result nil)
          (defun-declarations-alist
           '((qa-defun-decl-live-p qa-defun-decl-live-p-handler)
             (qa-defun-decl-live-q qa-defun-decl-live-q-handler)
             (qa-defun-decl-live-r qa-defun-decl-live-r-handler))))
      (eval (list 'defun 'qa-defun-decl-live-list nil
                  (cons 'declare qa-defun-decl-live-sequence)
                  42))
      qa-defun-decl-live-result)
    'r)

(qa (let ((defun-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler)))
          (qa-defun-decl-seen nil))
      (eval (list 'defun 'qa-defun-decl-vector nil
                  (cons 'declare (vector '(qa-defun-decl-test first second)))
                  42))
      qa-defun-decl-seen)
    '(qa-defun-decl-vector nil (first second)))

(qa-err-data
 (let ((defun-declarations-alist
        '((qa-defun-decl-test qa-defun-decl-handler))))
   (eval '(defun qa-defun-decl-dotted ()
            (declare (qa-defun-decl-test . bad))
            42))
   'returned)
 'wrong-type-argument '(listp bad))

(qa-err-data
 (let ((defun-declarations-alist
        '((qa-defun-decl-test qa-defun-decl-handler))))
   (eval '(defun qa-defun-decl-dotted-clauses ()
            (declare . bad)
            42)))
 'wrong-type-argument '(sequencep bad))

(qa-err-data
 (eval '(defun qa-defun-decl-non-list-spec ()
          (declare 5)
          42))
 'wrong-type-argument '(listp 5))
(qa-err-data
 (eval '(defun qa-defun-decl-vector-spec ()
          (declare [5])
          42))
 'wrong-type-argument '(listp [5]))

(qa-err-data
 (let ((defun-declarations-alist
        '((qa-defun-decl-test . qa-defun-decl-handler))))
   (eval '(defun qa-defun-decl-dotted-handler-entry (x)
            (declare (qa-defun-decl-test first))
            x)))
 'wrong-type-argument '(listp qa-defun-decl-handler))

(qa (let ((defun-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler))))
      (let* ((values (list 'first))
             (spec (cons 'qa-defun-decl-test values))
             (declaration (list 'declare spec))
             (form (list 'defun 'qa-defun-decl-circular nil declaration 42)))
        (setcdr values values)
        (condition-case err
            (progn (eval form) 'returned)
          (circular-list
           (list (car err) (eq (cadr err) values))))))
    '(circular-list t))

(qa (let ((defun-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler))))
      (let* ((clauses (list '(qa-defun-decl-test first)))
             (declaration (cons 'declare clauses))
             (form (list 'defun 'qa-defun-decl-circular-clauses
                         nil declaration 42)))
        (setcdr clauses clauses)
        (condition-case err
            (progn (eval form) 'returned)
          (circular-list
           (list (car err) (eq (cadr err) clauses))))))
    '(circular-list t))

(qa-err-data
 (let ((macro-declarations-alist
        '((qa-defun-decl-test qa-defun-decl-handler))))
   (eval '(defmacro qa-defmacro-decl-dotted ()
            (declare (qa-defun-decl-test . bad))
            42)))
 'wrong-type-argument '(listp bad))

;; Handler errors occur before `defalias' and preserve previous cells.
(defun qa-defun-decl-atomicity () 'old)
(qa (let ((defun-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler))))
      (condition-case nil
          (eval '(defun qa-defun-decl-atomicity ()
                   (declare (qa-defun-decl-test . bad))
                   'new))
        (wrong-type-argument nil))
      (qa-defun-decl-atomicity))
    'old)

(defmacro qa-defmacro-decl-atomicity () 'old)
(qa (let ((macro-declarations-alist
           '((qa-defun-decl-test qa-defun-decl-handler))))
      (condition-case nil
          (eval '(defmacro qa-defmacro-decl-atomicity ()
                   (declare (qa-defun-decl-test . bad))
                   'new))
        (wrong-type-argument nil))
      (macroexpand '(qa-defmacro-decl-atomicity)))
    'old)

;; Declaration expansion precedes `defalias' validating a non-symbol name.
(qa-err-data
 (let ((defun-declarations-alist
        '((qa-defun-decl-test qa-defun-decl-handler))))
   (eval '(defun 5 () (declare (qa-defun-decl-test . bad)) 42)))
 'wrong-type-argument '(listp bad))
(princ (qa--done))
