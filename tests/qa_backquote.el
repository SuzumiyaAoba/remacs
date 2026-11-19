;; backquote / unquote / splice — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - `x quotes, ,x unquotes, ,@x splices.
;;   - Nested ` and , nest properly.
;;   - ,. was REMOVED from the reader in Emacs 31 — can't write it.
;;
;; `qa' evals its form in DYNAMIC scope — locals referenced inside a
;; `qa' form must be defvar'd (special) to be visible to `eval'.
(defvar x)
(defvar y)
(defvar l)
(defvar a)
(defvar b)
(defvar f)
(defvar k)
(defvar v)
(defvar r)
(defvar l1)
(defvar l2)
(defvar show)

;; == basic backquote — same as quote when no , present ==
(qa `(a b c) '(a b c))
(qa `(1 2 3) '(1 2 3))

;; Internal LEVEL values retain GNU's lazy validation and numeric types.
(qa (backquote-process (list backquote-unquote-symbol 'x) 0.5)
    (list 0 'quote (list backquote-unquote-symbol 'x)))
(qa (backquote-process (list backquote-unquote-symbol 'x) 18446744073709551616)
    (list 0 'quote (list backquote-unquote-symbol 'x)))
(qa (with-temp-buffer
      (let ((level (point-marker)))
        (list
         (backquote-process (list backquote-unquote-symbol 'x) level)
         (backquote-process
          (list backquote-backquote-symbol
                (list backquote-unquote-symbol 'x))
          level))))
    (list (list 0 'quote (list backquote-unquote-symbol 'x))
          (list 0 'quote
                (list backquote-backquote-symbol
                      (list backquote-unquote-symbol 'x)))))
(qa (backquote-process '(x) "unused") '(0 quote (x)))
(qa (let ((backquote-unquote-symbol nil))
      (backquote-process (list (intern ",") 'x)))
    (list 0 'quote (list (intern ",") 'x)))
(qa (let ((backquote-splice-symbol nil))
      (backquote-process (list (intern ",@") 'x)))
    (list 0 'quote (list (intern ",@") 'x)))
(qa (let* ((marker (list 'marker))
           (backquote-unquote-symbol marker))
      (backquote-process (cons marker (list 'x))))
    '(1 . x))
(qa (let* ((symbols-with-pos-enabled t)
           (form (car (read-positioning-symbols "(`,x)")))
           (template (cadr form))
           (result (backquote-process template)))
      (list (car result) (eq (cdr result) (cadr template))))
    '(1 t))
(qa-err-data
 (backquote-process (list backquote-unquote-symbol 'x) "oops")
 'wrong-type-argument '(number-or-marker-p "oops"))
(qa-err-data
 (backquote-delay-process
  (list backquote-unquote-symbol
        (list backquote-unquote-symbol 'x))
  "oops")
 'wrong-type-argument '(number-or-marker-p "oops"))
(qa-err-data
 (backquote-process (list backquote-unquote-symbol 'x) (make-marker))
 'error '("Marker does not point anywhere"))

;; == ,x unquotes ==
(let ((x 42))
  (qa `(a ,x c) '(a 42 c)))

;; == ,@ splices ==
(let ((l (list 1 2)))
  (qa `(a ,@l c) '(a 1 2 c)))

;; == ,@ at start ==
(let ((l (list 1 2)))
  (qa `(,@l c) '(1 2 c)))

;; == ,@ at end ==
(let ((l (list 2 3)))
  (qa `(a ,@l) '(a 2 3)))

;; == ,@ of nil — no elements ==
(let ((l nil))
  (qa `(a ,@l c) '(a c)))

;; == ,@ in middle splices inline ==
(let ((l (list 'x 'y)))
  (qa `(p ,@l q) '(p x y q)))

;; == multiple splices ==
(let ((a (list 1)) (b (list 2)))
  (qa `(,@a m ,@b) '(1 m 2)))
;; == nested backquote ==
;; , inside ` inside ` refers to the INNER level (literal ` keeps it)
(let ((x 5))
  (qa `(outer `(inner ,x)) '(outer `(inner ,x))))

;; == inner ,x in nested ` does NOT see outer's x ==
;; The inner ` is a distinct quoting level — ,x inside it stays literal.
(let ((x 5))
  (qa `(a `(b ,x)) '(a `(b ,x))))

;; == , inside a vector inside backquote ==
(let ((x 5))
  (qa `(a [,x] c) '(a [5] c)))

;; == , inside dotted tail ==
(let ((x 5))
  (qa `(a . ,x) '(a . 5)))

;; == backquote produces a fresh list each time ==
(let* ((l1 `(1 2 3))
       (l2 `(1 2 3)))
  (qa (eq l1 l2) nil)
  (qa (equal l1 l2) t))

;; == , on non-list returns the value ==
(qa `(a ,(list 1 2) c) '(a (1 2) c))

;; == double , — the inner is unquote of unquote ==
(let ((x 'y) (y 'val))
  ;; ,,x → ,y → val? actually ,,x is `',x' — evaluates to the symbol
  ;; that ,x evaluated to.
  (qa ``(a ,,x) '`(a ,y)))

;; == backquote on atom ==
(qa `42 42)
(qa `sym 'sym)
(qa `"str" "str")

;; == comma inside quote — literal ==
(qa '`(a ,b) '`(a ,b))

;; == reader accepts ",x" as a form ==
(let ((r (read-from-string "`,x")))
  ;; reads to (\` (\, x))
  (qa (consp (car r)) t))

;; == , on a function call evaluates it ==
(let ((f (lambda (x) (* x 2))))
  (qa `(a ,(funcall f 5)) '(a 10)))

;; == ,@ on result of fn ==
(let ((f (lambda () (list 1 2))))
  (qa `(a ,@(funcall f)) '(a 1 2)))

;; == nested splicing with conditional ==
(let ((show t))
  (qa `(a ,@(when show '(b)) c) '(a b c)))
(let ((show nil))
  (qa `(a ,@(when show '(b)) c) '(a c)))

;; == backquote in a function return ==
(let ((f (lambda (x) `(sym ,x))))
  (qa (funcall f 'val) '(sym val)))

;; == combination: list-building with `, ==
(let ((k 'key) (v 'val))
  (qa `((,k . ,v) (other . stuff))
      '((key . val) (other . stuff))))

;; == unquote doesn't copy when used on variable ==
;; `,x is equivalent to x, not a fresh cons
(let* ((l (list 1 2 3))
       (r `(a ,l)))
  ;; the inner list IS l
  (qa (eq (cadr r) l) t))

;; GNU 31.1 exceeded a 1 GiB RSS limit on a circular template.
;; Remacs detects the cycle before its Rust traversal can grow without bound.
(when qa--remacs
  (qa (let* ((template (list 'a))
             (form (list 'backquote template)))
        (setcdr template template)
        (condition-case err
            (macroexpand form)
          (circular-list (list (car err) (eq (cadr err) template)))))
      '(circular-list t))
  (qa (let* ((template (list nil))
             (form (list 'backquote template)))
        (setcar template template)
        (condition-case err
            (macroexpand form)
          (circular-list (list (car err) (eq (cadr err) template)))))
      '(circular-list t))
  (qa (let* ((template (vector nil))
             (form (list 'backquote template)))
        (aset template 0 template)
        (condition-case err
            (macroexpand form)
          (circular-list (list (car err) (eq (cadr err) template)))))
      '(circular-list t))
  ;; Unquote arity and the internal list helpers also walk raw lists.
  (qa (let* ((template (list 'comma nil))
             (form (list 'backquote template)))
        (setcdr (cdr template) template)
        (condition-case err
            (macroexpand form)
          (circular-list (list (car err) (eq (cadr err) template)))))
      '(circular-list t))
  (qa (let ((items (list (list 0 'a))))
        (setcdr items items)
        (condition-case err
            (backquote-listify items nil)
          (circular-list (list (car err) (eq (cadr err) items)))))
      '(circular-list t))
  (qa (let* ((item (list 0 'a))
             (items (list item)))
        (setcdr (cdr item) item)
        (condition-case err
            (backquote-listify items nil)
          (circular-list (list (car err) (eq (cadr err) item)))))
      '(circular-list t)))

;; Active-path cycle detection must still allow shared acyclic subforms.
(qa (let* ((shared (list 'a))
           (template (list shared shared))
           (form (list 'backquote template)))
      (equal (macroexpand form) (list 'quote template)))
    t)

(princ (qa--done))
