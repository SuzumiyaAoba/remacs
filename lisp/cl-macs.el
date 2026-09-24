;;; cl-macs.el --- subset of GNU cl-macs/cl-lib -*- lexical-binding: t -*-

;; Port of the GNU Emacs 31 cl-macs.el/cl-lib.el generalized-variable
;; macros plus the predicate helpers their expanders use.
;; `macroexp' helpers live in the prelude (which stands in for GNU's
;; dumped macroexp.el), so no explicit require is needed here.

;;; Code:

;;; Predicates for analyzing Lisp forms (GNU cl-macs.el).

(defconst cl--simple-funcs '(car cdr nth aref elt if and or + - 1+ 1- min max
			    car-safe cdr-safe progn prog1 prog2))
(defconst cl--safe-funcs '(* / % length memq list vector vectorp
			  < > <= >= = error))

(defun cl--simple-expr-p (x &optional size)
  "Check if no side effects, and executes quickly."
  (or size (setq size 10))
  (if (and (consp x) (not (memq (car x) '(quote function cl-function))))
      (and (symbolp (car x))
	   (or (memq (car x) cl--simple-funcs)
	       (get (car x) 'side-effect-free))
	   (progn
	     (setq size (1- size))
	     (while (and (setq x (cdr x))
			 (setq size (cl--simple-expr-p (car x) size))))
	     (and (null x) (>= size 0) size)))
    (and (> size 0) (1- size))))

(defun cl--simple-exprs-p (xs)
  "Map `cl--simple-expr-p' to each element of list XS."
  (while (and xs (cl--simple-expr-p (car xs)))
    (setq xs (cdr xs)))
  (not xs))

(defun cl--safe-expr-p (x)
  "Check if no side effects."
  (or (not (and (consp x) (not (memq (car x) '(quote function cl-function)))))
      (and (symbolp (car x))
	   (or (memq (car x) cl--simple-funcs)
	       (memq (car x) cl--safe-funcs)
	       (get (car x) 'side-effect-free))
	   (progn
	     (while (and (setq x (cdr x)) (cl--safe-expr-p (car x))))
	     (null x)))))

(defun cl--expr-contains (x y)
  "Count number of times X refers to Y.  Return nil for 0 times."
  ;; FIXME: This is naive, and it will cl-count Y as referred twice in
  ;; (let ((Y 1)) Y) even though it should be 0.  Also it is often called on
  ;; non-macroexpanded code, so it may also miss some occurrences that would
  ;; only appear in the expanded code.
  (cond ((equal y x) 1)
	((and (consp x) (not (memq (car x) '(quote function cl-function))))
	 (let ((sum 0))
	   (while (consp x)
	     (setq sum (+ sum (or (cl--expr-contains (pop x) y) 0))))
	   (setq sum (+ sum (or (cl--expr-contains x y) 0)))
	   (and (> sum 0) sum)))
	(t nil)))

(defun cl--expr-contains-any (x y)
  (while (and y (not (cl--expr-contains x (car y)))) (pop y))
  y)

(defun cl--expr-depends-p (x y)
  "Check whether X may depend on any of the symbols in Y."
  (and (not (macroexp-const-p x))
       (or (not (cl--safe-expr-p x)) (cl--expr-contains-any x y))))

;;; Generalized-variable helpers (GNU cl-lib.el).

(defun cl-list* (arg &rest rest)
  "Return a new list with specified ARGs as conses and last arg as tail.
Thus, (cl-list* A B C) is equivalent to (cons A (cons B C))."
  (declare (compiler-macro cl--compiler-macro-list*))
  (if (not rest) arg
    (let* ((args (reverse (cons arg rest)))
	   (form (car args)))
      (while (setq args (cdr args))
	(setq form (cons 'cons (cons (car args) (cons form nil)))))
      form)))

(defun cl--compiler-macro-list* (_form arg &rest others)
  (let* ((args (reverse (cons arg others)))
	 (form (car args)))
    (while (setq args (cdr args))
      (setq form `(cons ,(car args) ,form)))
    form))

;;; Aliases (GNU cl-lib.el / cl-macs.el).

(defalias 'cl-values #'list)
(defalias 'cl-nth-value #'nth)
(defalias 'cl-multiple-value-call #'apply)
(defalias 'cl-svref #'aref)
(defalias 'cl-copy-seq #'copy-sequence)

;; GNU has `(defalias 'cl-locally #'progn)'; our evaluator cannot call a
;; special form through a symbol alias, so expand to `progn' instead.
(defmacro cl-locally (&rest body)
  (cons 'progn body))

(defsubst cl-floatp-safe (object)
  "Return t if OBJECT is a floating point number."
  (floatp object))

;;; car/cdr accessor aliases (GNU cl-lib.el defines only the 3- and
;;; 4-deep accessors, cl-caaar .. cl-cddddr).

(defalias 'cl-caaar #'caaar)
(defalias 'cl-caadr #'caadr)
(defalias 'cl-cadar #'cadar)
(defalias 'cl-caddr #'caddr)
(defalias 'cl-cdaar #'cdaar)
(defalias 'cl-cdadr #'cdadr)
(defalias 'cl-cddar #'cddar)
(defalias 'cl-cdddr #'cdddr)
(defalias 'cl-caaaar #'caaaar)
(defalias 'cl-caaadr #'caaadr)
(defalias 'cl-caadar #'caadar)
(defalias 'cl-caaddr #'caaddr)
(defalias 'cl-cadaar #'cadaar)
(defalias 'cl-cadadr #'cadadr)
(defalias 'cl-caddar #'caddar)
(defalias 'cl-cadddr #'cadddr)
(defalias 'cl-cdaaar #'cdaaar)
(defalias 'cl-cdaadr #'cdaadr)
(defalias 'cl-cdadar #'cdadar)
(defalias 'cl-cdaddr #'cdaddr)
(defalias 'cl-cddaar #'cddaar)
(defalias 'cl-cddadr #'cddadr)
(defalias 'cl-cdddar #'cdddar)
(defalias 'cl-cddddr #'cddddr)

;;; Small cl-lib/cl-seq list helpers.

(defun cl-acons (key value alist)
  "Add KEY and VALUE to ALIST.
Return a new list with (cons KEY VALUE) as car and ALIST as cdr."
  (cons (cons key value) alist))

(defsubst cl-assoc-if (cl-pred cl-list &rest cl-keys)
  "Find the first item whose car satisfies PREDICATE in LIST.

Keywords supported:  :key"
  (let ((cl-key (car (cdr (memq :key cl-keys)))))
    (while (and cl-list
                (or (not (consp (car cl-list)))
                    (not (funcall cl-pred
                                  (if cl-key
                                      (funcall cl-key (car (car cl-list)))
                                    (car (car cl-list)))))))
      (setq cl-list (cdr cl-list)))
    (car cl-list)))

(defsubst cl-assoc-if-not (cl-pred cl-list &rest cl-keys)
  "Find the first item whose car does not satisfy PREDICATE in LIST.

Keywords supported:  :key"
  (let ((cl-key (car (cdr (memq :key cl-keys)))))
    (while (and cl-list
                (or (not (consp (car cl-list)))
                    (funcall cl-pred
                             (if cl-key
                                 (funcall cl-key (car (car cl-list)))
                               (car (car cl-list))))))
      (setq cl-list (cdr cl-list)))
    (car cl-list)))

;;; callf/callf2.

(defmacro cl-callf (func place &rest args)
  "Set PLACE to (FUNC PLACE ARGS...).
FUNC should be an unquoted function name or a lambda expression.
PLACE may be a symbol, or any generalized variable allowed by
`setf'."
  (declare (indent 2))
  (gv-letplace (getter setter) place
    (let* ((rargs (cons getter args)))
      (funcall setter
               (if (symbolp func) (cons func rargs)
                 `(funcall #',func ,@rargs))))))

(defmacro cl-callf2 (func arg1 place &rest args)
  "Set PLACE to (FUNC ARG1 PLACE ARGS...).
Like `cl-callf', but PLACE is the second argument of FUNC, not the first.

\(fn FUNC ARG1 PLACE ARGS...)"
  (declare (indent 3))
  (if (and (cl--safe-expr-p arg1) (cl--simple-expr-p place) (symbolp func))
      `(setf ,place (,func ,arg1 ,place ,@args))
    (macroexp-let2 nil a1 arg1
      (gv-letplace (getter setter) place
        (let* ((rargs (cl-list* a1 getter args)))
          (funcall setter
                   (if (symbolp func) (cons func rargs)
                     `(funcall #',func ,@rargs))))))))

;;; Parallel assignment and list surgery.

(defmacro cl-psetf (&rest args)
  "Set PLACEs to the values VALs in parallel.
This is like `setf', except that all VAL forms are evaluated (in order)
before assigning any PLACEs to the corresponding values.

\(fn PLACE VAL PLACE VAL ...)"
  (let ((p args) (simple t) (vars nil))
    (while p
      (if (or (not (symbolp (car p))) (cl--expr-depends-p (nth 1 p) vars))
	  (setq simple nil))
      (if (memq (car p) vars)
	  (error "Destination duplicated in psetf: %s" (car p)))
      (push (pop p) vars)
      (or p (error "Odd number of arguments to cl-psetf"))
      (pop p))
    (if simple
	`(progn (setq ,@args) nil)
      (setq args (reverse args))
      (let ((expr `(setf ,(cadr args) ,(car args))))
	(while (setq args (cddr args))
	  (setq expr `(setf ,(cadr args) (prog1 ,(car args) ,expr))))
	`(progn ,expr nil)))))

(defmacro cl-remf (place tag)
  "Remove TAG from property list PLACE.
PLACE may be a symbol, or any generalized variable allowed by `setf'.
The form returns true if TAG was found and removed, nil otherwise."
  (gv-letplace (tval setter) place
    (macroexp-let2 macroexp-copyable-p ttag tag
      `(if (eq ,ttag (car ,tval))
           (progn ,(funcall setter `(cddr ,tval))
                  t)
         (cl--do-remf ,tval ,ttag)))))

(defun cl--do-remf (plist tag)
  (let ((p (cdr plist)))
    ;; Can't use `plist-member' here because it goes to the cons-cell
    ;; of TAG and we need the one before.
    (while (and (cdr p) (not (eq (car (cdr p)) tag))) (setq p (cdr (cdr p))))
    (and (cdr p) (progn (setcdr p (cdr (cdr (cdr p)))) t))))

(defmacro cl-shiftf (place &rest args)
  "Shift left among PLACEs.
Example: (cl-shiftf A B C) sets A to B, B to C, and returns the old A.
Each PLACE may be a symbol, or any generalized variable allowed by `setf'.

\(fn PLACE... VAL)"
  (cond
   ((null args) place)
   ((symbolp place) `(prog1 ,place (setq ,place (cl-shiftf ,@args))))
   (t
    (gv-letplace (getter setter) place
      `(prog1 ,getter
         ,(funcall setter `(cl-shiftf ,@args)))))))

(defmacro cl-rotatef (&rest args)
  "Rotate left among PLACEs.
Example: (cl-rotatef A B C) sets A to B, B to C, and C to A.  It returns nil.
Each PLACE may be a symbol, or any generalized variable allowed by `setf'.

\(fn PLACE...)"
  (if (not (memq nil (mapcar #'symbolp args)))
      (and (cdr args)
	   (let ((sets nil)
		 (first (car args)))
	     (while (cdr args)
	       (setq sets (nconc sets (list (pop args) (car args)))))
	     `(cl-psetf ,@sets ,(car args) ,first)))
    (let* ((places (reverse args))
	   (temp (make-symbol "--cl-rotatef--"))
	   (form temp))
      (while (cdr places)
        (setq form
              (gv-letplace (getter setter) (pop places)
                `(prog1 ,getter ,(funcall setter form)))))
      (gv-letplace (getter setter) (car places)
	(macroexp-let* `((,temp ,getter))
                       `(progn ,(funcall setter form) nil))))))

(defmacro cl-pushnew (x place &rest keys)
  "Add X to the list stored in PLACE unless X is already in the list.
PLACE is a generalized variable that stores a list.

Like (push X PLACE), except that PLACE is unmodified if X is `eql'
to an element already in the list stored in PLACE.

\nKeywords supported:  :test :test-not :key
\n(fn X PLACE [KEYWORD VALUE]...)"
  (if (symbolp place)
      (if (null keys)
          (macroexp-let2 nil var x
            `(if (memql ,var ,place)
                 ;; This symbol may later on expand to actual code which then
                 ;; trigger warnings like "value unused" since cl-pushnew's
                 ;; return value is rarely used.  It should not matter that
                 ;; other warnings may be silenced, since `place' is used
                 ;; earlier and should have triggered them already.
                 (with-no-warnings ,place)
               (setq ,place (cons ,var ,place))))
	`(setq ,place (cl-adjoin ,x ,place ,@keys)))
    `(cl-callf2 cl-adjoin ,x ,place ,@keys)))

;;;; From GNU cl-lib.el.

(defvar cl--optimize-speed 1)
(defvar cl--optimize-safety 1)

(defvar cl-custom-print-functions nil
  "This is a list of functions that format user objects for printing.
Each function is called in turn with three arguments: the object, the
stream, and the print level (currently ignored).  If it is able to
print the object it returns true; otherwise it returns nil and the
printer proceeds to the next function on the list.

This variable is not used at present, but it is defined in hopes that
a future Emacs interpreter will be able to use it.")

(defun cl--set-buffer-substring (start end val)
  "Delete region from START to END and insert VAL."
  (save-excursion (delete-region start end)
		  (goto-char start)
		  (insert val)
		  val))

(defun cl--set-substring (str start end val)
  (if end (if (< end 0) (cl-incf end (length str)))
    (setq end (length str)))
  (if (< start 0) (cl-incf start (length str)))
  (concat (and (> start 0) (substring str 0 start))
	  val
	  (and (< end (length str)) (substring str end))))

(gv-define-expander substring
  (lambda (do place from &optional to)
    (gv-letplace (getter setter) place
      (macroexp-let2* nil ((start from) (end to))
        (funcall do `(substring ,getter ,start ,end)
                 (lambda (v)
                   (macroexp-let2 nil v v
                     `(progn
                        ,(funcall setter `(cl--set-substring
                                           ,getter ,start ,end ,v))
                        ,v))))))))

;;; Blocks and exits.

(defalias 'cl--block-wrapper 'identity)
(defalias 'cl--block-throw 'throw)

;;; Multiple values.

(defun cl--defalias (cl-f el-f &optional doc)
  "Define function CL-F as definition EL-F.
Like `defalias' but marks the alias itself as inlinable."
  (defalias cl-f el-f doc)
  (put cl-f 'byte-optimizer 'byte-compile-inline-expand))

(defun cl-values-list (list)
  "Return multiple values, Common Lisp style, taken from a list.
LIST specifies the list of values that the containing function
should return.

Note that Emacs Lisp doesn't really support multiple values, so
all this function does is return LIST."
  (unless (listp list)
    (signal 'wrong-type-argument (list list)))
  list)

(defsubst cl-multiple-value-list (expression)
  "Return a list of the multiple values produced by EXPRESSION.
This handles multiple values in Common Lisp style, but it does not
work right when EXPRESSION calls an ordinary Emacs Lisp function
that returns just one value."
  expression)

(defsubst cl-multiple-value-apply (function expression)
  "Evaluate EXPRESSION to get multiple values and apply FUNCTION to them.
This handles multiple values in Common Lisp style, but it does not work
right when EXPRESSION calls an ordinary Emacs Lisp function that returns just
one value."
  (apply function expression))

(defvar cl--proclaims-deferred nil)

(defun cl-proclaim (spec)
  "Record a global declaration specified by SPEC."
  (if (fboundp 'cl--do-proclaim) (cl--do-proclaim spec t)
    (push spec cl--proclaims-deferred))
  nil)

(defmacro cl-declaim (&rest specs)
  "Like `cl-proclaim', but takes any number of unevaluated, unquoted arguments.
Puts `(cl-eval-when (compile load eval) ...)' around the declarations
so that they are registered at compile-time as well as run-time."
  (let ((body (mapcar (lambda (x) `(cl-proclaim ',x)) specs)))
    `(progn ,@body)))

;;; Numbers.

(defconst cl-digit-char-table
  (let* ((digits (make-vector 256 nil))
         (populate (lambda (start end base)
                     (mapc (lambda (i)
                             (aset digits i (+ base (- i start))))
                           (number-sequence start end)))))
    (funcall populate ?0 ?9 0)
    (funcall populate ?A ?Z 10)
    (funcall populate ?a ?z 10)
    digits))

(defun cl-digit-char-p (char &optional radix)
  "Test if CHAR is a digit in the specified RADIX (default 10).
If true return the decimal value of digit CHAR in RADIX."
  (or (<= 2 (or radix 10) 36)
      (signal 'args-out-of-range (list 'radix radix '(2 36))))
  (let ((n (aref cl-digit-char-table char)))
    (and n (< n (or radix 10)) n)))

(defconst cl-most-positive-float nil
  "The largest value that a Lisp float can hold.
If your system supports infinities, this is the largest finite value.
For Emacs, this equals 1.7976931348623157e+308.
Call `cl-float-limits' to set this.")

(defconst cl-most-negative-float nil
  "The largest negative value that a Lisp float can hold.
This is simply -`cl-most-positive-float'.
Call `cl-float-limits' to set this.")

(defconst cl-least-positive-float nil
  "The smallest value greater than zero that a Lisp float can hold.
For Emacs, this equals 5e-324 if subnormal numbers are supported,
`cl-least-positive-normalized-float' if they are not.
Call `cl-float-limits' to set this.")

(defconst cl-least-negative-float nil
  "The smallest value less than zero that a Lisp float can hold.
This is simply -`cl-least-positive-float'.
Call `cl-float-limits' to set this.")

(defconst cl-least-positive-normalized-float nil
  "The smallest normalized Lisp float greater than zero.
For Emacs, this equals 2.2250738585072014e-308.
Call `cl-float-limits' to set this.")

(defconst cl-least-negative-normalized-float nil
  "The largest normalized Lisp float less than zero.
This is simply -`cl-least-positive-normalized-float'.
Call `cl-float-limits' to set this.")

(defconst cl-float-epsilon nil
  "The smallest positive float which adds to 1.0.
For Emacs, this equals 2.220446049250313e-16.
Call `cl-float-limits' to set this.")

(defconst cl-float-negative-epsilon nil
  "The smallest positive value which subtracts from 1.0.
For Emacs, this equals 1.1102230246251565e-16.
Call `cl-float-limits' to set this.")

;;; Sequence functions.

(defun cl-mapcar (cl-func cl-x &rest cl-rest)
  "Apply FUNCTION to each element of SEQ, and make a list of the results.
If there are several SEQs, FUNCTION is called with that many arguments,
and mapping stops as soon as the shortest list runs out.  With just one
SEQ, this is like `mapcar'.  With several, it is like the Common Lisp
`mapcar' function extended to ordinary sequence types.
\n(fn FUNCTION SEQ...)"
  (if cl-rest
      (if (or (cdr cl-rest) (nlistp cl-x) (nlistp (car cl-rest)))
	  (cl--mapcar-many cl-func (cons cl-x cl-rest) 'accumulate)
	(let ((cl-res nil)
	      (cl-x1 cl-x)
	      (cl-y1 (car cl-rest)))
	  (while (and cl-x1 cl-y1)
	    (push (funcall cl-func (pop cl-x1) (pop cl-y1)) cl-res))
	  (nreverse cl-res)))
    (mapcar cl-func cl-x)))

(defun cl-ldiff (list sublist)
  "Return a copy of LIST with the tail SUBLIST removed."
  (let ((res nil))
    (while (and (consp list) (not (eq list sublist)))
      (push (pop list) res))
    (nreverse res)))

(defun cl-copy-list (list)
  "Return a copy of LIST, which may be a dotted list.
The elements of LIST are not copied, just the list structure itself."
  (declare (side-effect-free error-free))
  (if (consp list)
      (let ((res nil))
	(while (consp list) (push (pop list) res))
	(prog1 (nreverse res) (setcdr res list)))
    (car list)))

(defun cl-adjoin (cl-item cl-list &rest cl-keys)
  "Return ITEM consed onto the front of LIST only if it's not already there.
Otherwise, return LIST unmodified.
\nKeywords supported:  :test :test-not :key
\n(fn ITEM LIST [KEYWORD VALUE]...)"
  (cond ((or (equal cl-keys '(:test eq))
	     (and (null cl-keys) (not (numberp cl-item))))
	 (if (memq cl-item cl-list) cl-list (cons cl-item cl-list)))
	((or (equal cl-keys '(:test equal)) (null cl-keys))
	 (if (member cl-item cl-list) cl-list (cons cl-item cl-list)))
	(t (apply 'cl--adjoin cl-item cl-list cl-keys))))

(defun cl-subst (cl-new cl-old cl-tree &rest cl-keys)
  "Substitute NEW for OLD everywhere in TREE (non-destructively).
Return a copy of TREE with all elements `eql' to OLD replaced by NEW.
\nKeywords supported:  :test :test-not :key
\n(fn NEW OLD TREE [KEYWORD VALUE]...)"
  (if (or cl-keys (and (numberp cl-old) (not (integerp cl-old))))
      (apply 'cl-sublis (list (cons cl-old cl-new)) cl-tree cl-keys)
    (cl--do-subst cl-new cl-old cl-tree)))

(defun cl--do-subst (cl-new cl-old cl-tree)
  (cond ((eq cl-tree cl-old) cl-new)
	((consp cl-tree)
	 (let ((a (cl--do-subst cl-new cl-old (car cl-tree)))
	       (d (cl--do-subst cl-new cl-old (cdr cl-tree))))
	   (if (and (eq a (car cl-tree)) (eq d (cdr cl-tree)))
	       cl-tree (cons a d))))
	(t cl-tree)))

(defun cl-pairlis (keys values &optional alist)
  "Make an alist from KEYS and VALUES.
Return a new alist composed by associating KEYS to corresponding VALUES;
the process stops as soon as KEYS or VALUES run out.
If ALIST is non-nil, the new pairs are prepended to it."
  (nconc (cl-mapcar 'cons keys values) alist))

(defun cl-constantly (value)
  "Return a function that takes any number of arguments, but returns VALUE."
  (lambda (&rest _)
    value))

(provide 'cl-macs)
;;; cl-macs.el ends here
