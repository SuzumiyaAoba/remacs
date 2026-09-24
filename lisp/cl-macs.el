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

;;; Macro-writing helpers (GNU cl-macs.el).

(defvar cl--gensym-counter 0)
(defun cl-gensym (&optional prefix)
  "Generate a new uninterned symbol.
The name is made by appending a number to PREFIX, default \"G\"."
  (declare (obsolete gensym "31.1"))
  (let ((pfix (if (stringp prefix) prefix "G"))
	(num (if (integerp prefix) prefix
	       (prog1 cl--gensym-counter
		 (setq cl--gensym-counter (1+ cl--gensym-counter))))))
    (make-symbol (format "%s%d" pfix num))))

(defvar cl--gentemp-counter 0)
(defun cl-gentemp (&optional prefix)
  "Generate a new interned symbol with a unique name.
The name is made by appending a number to PREFIX, default \"T\"."
  (let ((pfix (if (stringp prefix) prefix "T"))
	name)
    (while (intern-soft (setq name (format "%s%d" pfix cl--gentemp-counter)))
      (setq cl--gentemp-counter (1+ cl--gentemp-counter)))
    (intern name)))

(defmacro cl-with-gensyms (names &rest body)
  "Bind each of NAMES to an uninterned symbol and evaluate BODY."
  (declare (debug (sexp body)) (indent 1))
  `(let ,(cl-loop for name in names collect
                  `(,name (gensym (symbol-name ',name))))
     ,@body))

(defmacro cl-once-only (names &rest body)
  "Generate code to evaluate each of NAMES just once in BODY.

This macro helps with writing other macros.  Each of NAMES is
either (NAME FORM) or NAME, which latter means (NAME NAME).
During macroexpansion, each NAME is bound to an uninterned
symbol.  The expansion evaluates each FORM and binds it to the
corresponding uninterned symbol.

For example, consider this macro:

    (defmacro my-cons (x)
      (cl-once-only (x)
        \\=`(cons ,x ,x)))

The call (my-cons (pop y)) will expand to something like this:

    (let ((g1 (pop y)))
      (cons g1 g1))

The use of `cl-once-only' ensures that the pop is performed only
once, as intended.

See also `macroexp-let2'."
  (declare (debug (sexp body)) (indent 1))
  (setq names (mapcar #'ensure-list names))
  (let ((our-gensyms (cl-loop for _ in names collect (gensym))))
    ;; During macroexpansion, obtain a gensym for each NAME.
    `(let ,(cl-loop for sym in our-gensyms collect `(,sym (gensym)))
       ;; Evaluate each FORM and bind to the corresponding gensym.
       ;;
       ;; We require this explicit call to `list' rather than using
       ;; (,,@(cl-loop ...)) due to a limitation of Elisp's backquote.
       `(let ,(list
               ,@(cl-loop for name in names for gensym in our-gensyms
                          for to-eval = (or (cadr name) (car name))
                          collect ``(,,gensym ,,to-eval)))
          ;; During macroexpansion, bind each NAME to its gensym.
          ,(let ,(cl-loop for name in names for gensym in our-gensyms
                          collect `(,(car name) ,gensym))
             ,@body)))))

;;; Loop and binding constructs (GNU cl-macs.el).

(defmacro cl-psetq (&rest args)
  "Set SYMs to the values VALs in parallel.
This is like `setq', except that all VAL forms are evaluated (in order)
before assigning any symbols SYM to the corresponding values.

\(fn SYM VAL SYM VAL ...)"
  (declare (debug setq))
  (cons 'cl-psetf args))

(defmacro cl-progv (symbols values &rest body)
  "Bind SYMBOLS to VALUES dynamically in BODY.
The forms SYMBOLS and VALUES are evaluated, and must evaluate to lists.
Each symbol in the first list is bound to the corresponding value in the
second list (or to nil if VALUES is shorter than SYMBOLS); then the
BODY forms are executed and their result is returned.  This is much like
a `let' form, except that the list of symbols can be computed at run-time."
  (declare (indent 2) (debug (form form def-body)))
  (let ((bodyfun (make-symbol "body"))
        (binds (make-symbol "binds"))
        (syms (make-symbol "syms"))
        (vals (make-symbol "vals")))
    `(progn
       (let* ((,syms ,symbols)
              (,vals ,values)
              (,bodyfun (lambda () ,@body))
              (,binds ()))
         (while ,syms
           (push (list (pop ,syms) (list 'quote (pop ,vals))) ,binds))
         (eval (list 'let (nreverse ,binds)
                     (list 'funcall (list 'quote ,bodyfun))))))))

(defmacro cl-do (steps endtest &rest body)
  "Bind variables and run BODY forms until END-TEST returns non-nil.
First, each VAR is bound to the associated INIT value as if by a `let' form.
Then, in each iteration of the loop, the END-TEST is evaluated; if true,
the loop is finished.  Otherwise, the BODY forms are evaluated, then each
VAR is set to the associated STEP expression (as if by a `cl-psetq' form)
and the next iteration begins.

Once the END-TEST becomes true, the RESULT forms are evaluated (with
the VARs still bound to their values) to produce the result
returned by `cl-do'.

Note that the entire loop is enclosed in an implicit nil block, so
that you can use `cl-return' to exit at any time.

Also note that END-TEST is checked before evaluating BODY.  If END-TEST is
initially non-nil, `cl-do' will exit without running BODY.

For more details, see `cl-do' description in Info node `(cl) Iteration'.

\(fn ((VAR INIT [STEP])...) (END-TEST [RESULT...]) BODY...)"
  (declare (indent 2)
           (debug
            ((&rest &or symbolp (symbolp &optional form form))
             (form body)
             cl-declarations body)))
  (cl--expand-do-loop steps endtest body nil))

(defmacro cl-do* (steps endtest &rest body)
  "Bind variables and run BODY forms until END-TEST returns non-nil.
First, each VAR is bound to the associated INIT value as if by a `let*' form.
Then, in each iteration of the loop, the END-TEST is evaluated; if true,
the loop is finished.  Otherwise, the BODY forms are evaluated, then each
VAR is set to the associated STEP expression (as if by a `setq'
form) and the next iteration begins.

Once the END-TEST becomes true, the RESULT forms are evaluated (with
the VARs still bound to their values) to produce the result
returned by `cl-do*'.

Note that the entire loop is enclosed in an implicit nil block, so
that you can use `cl-return' to exit at any time.

Also note that END-TEST is checked before evaluating BODY.  If END-TEST is
initially non-nil, `cl-do*' will exit without running BODY.

This is to `cl-do' what `let*' is to `let'.
For more details, see `cl-do*' description in Info node `(cl) Iteration'.

\(fn ((VAR INIT [STEP])...) (END-TEST [RESULT...]) BODY...)"
  (declare (indent 2) (debug cl-do))
  (cl--expand-do-loop steps endtest body t))

(defun cl--expand-do-loop (steps endtest body star)
  `(cl-block nil
     (,(if star 'let* 'let)
      ,(mapcar (lambda (c) (if (consp c) (list (car c) (nth 1 c)) c))
               steps)
      (while (not ,(car endtest))
        ,@body
        ,@(let ((sets (mapcar (lambda (c)
                                (and (consp c) (cdr (cdr c))
                                     (list (car c) (nth 2 c))))
                              steps)))
            (setq sets (delq nil sets))
            (and sets
                 (list (cons (if (or star (not (cdr sets)))
                                 'setq 'cl-psetq)
                             (apply #'append sets))))))
      ,@(or (cdr endtest) '(nil)))))

(defvar cl--tagbody-alist nil)

(defmacro cl-tagbody (&rest labels-or-stmts)
  "Execute statements while providing for control transfers to labels.
Each element of LABELS-OR-STMTS can be either a label (integer or symbol)
or a `cons' cell, in which case it's taken to be a statement.
This distinction is made before performing macroexpansion.
Statements are executed in sequence left to right, discarding any return value,
stopping only when reaching the end of LABELS-OR-STMTS.
Any statement can transfer control at any time to the statements that follow
one of the labels with the special form (go LABEL).
Labels have lexical scope and dynamic extent."
  (let ((blocks '())
        (first-label (if (consp (car labels-or-stmts))
                       'cl--preamble (pop labels-or-stmts))))
    (let ((block (list first-label)))
      (dolist (label-or-stmt labels-or-stmts)
        (if (consp label-or-stmt) (push label-or-stmt block)
          ;; Add a "go to next block" to implement the fallthrough.
          (unless (eq 'go (car-safe (car-safe block)))
            (push `(go ,label-or-stmt) block))
          (push (nreverse block) blocks)
          (setq block (list label-or-stmt))))
      (unless (eq 'go (car-safe (car-safe block)))
        (push '(go cl--exit) block))
      (push (nreverse block) blocks))
    (let ((catch-tag (make-symbol "cl--tagbody-tag"))
          (cl--tagbody-alist cl--tagbody-alist))
      (push (cons 'cl--exit catch-tag) cl--tagbody-alist)
      (dolist (block blocks)
        (push (cons (car block) catch-tag) cl--tagbody-alist))
      (macroexpand-all
       `(let ((next-label ',first-label))
          (while
              (not (eq (setq next-label
                             (catch ',catch-tag
                               (cl-case next-label
                                 ,@blocks)))
                       'cl--exit))))
       `((go . ,(lambda (label)
                  (let ((catch-tag (cdr (assq label cl--tagbody-alist))))
                    (unless catch-tag
                      (error "Unknown cl-tagbody go label `%S'" label))
                    `(throw ',catch-tag ',label))))
         ,@macroexpand-all-environment)))))

(defun cl--prog (binder bindings body)
  (let (decls)
    (while (eq 'declare (car-safe (car body)))
      (push (pop body) decls))
    `(cl-block nil
       (,binder ,bindings
         ,@(nreverse decls)
         (cl-tagbody . ,body)))))

(defmacro cl-prog (bindings &rest body)
  "Run BODY like a `cl-tagbody' after setting up the BINDINGS.
Shorthand for (cl-block nil (let BINDINGS (cl-tagbody BODY)))"
  (cl--prog 'let bindings body))

(defmacro cl-prog* (bindings &rest body)
  "Run BODY like a `cl-tagbody' after setting up the BINDINGS.
Shorthand for (cl-block nil (let* BINDINGS (cl-tagbody BODY)))"
  (cl--prog 'let* bindings body))

(defmacro cl-do-symbols (spec &rest body)
  "Loop over all symbols.
Evaluate BODY with VAR bound to each interned symbol, or to each symbol
from OBARRAY.

\(fn (VAR [OBARRAY [RESULT]]) BODY...)"
  (declare (indent 1)
           (debug ((symbolp &optional form form) cl-declarations
                   def-body)))
  ;; Apparently this doesn't have an implicit block.
  `(cl-block nil
     (let (,(car spec))
       (mapatoms #'(lambda (,(car spec)) ,@body)
                 ,@(and (cadr spec) (list (cadr spec))))
       ,(nth 2 spec))))

(defmacro cl-do-all-symbols (spec &rest body)
  "Like `cl-do-symbols', but use the default obarray.

\(fn (VAR [RESULT]) BODY...)"
  (declare (indent 1) (debug ((symbolp &optional form) cl-declarations body)))
  `(cl-do-symbols (,(car spec) nil ,(cadr spec)) ,@body))

;;; Evaluation-time control.

(defmacro cl-eval-when (when &rest body)
  "Control when BODY is evaluated.
If `compile' is in WHEN, BODY is evaluated when compiled at top-level.
If `load' is in WHEN, BODY is evaluated when loaded after top-level compile.
If `eval' is in WHEN, BODY is evaluated when interpreted or at non-top-level.

\(fn (WHEN...) BODY...)"
  (declare (indent 1) (debug (sexp body)))
  (if (and (macroexp-compiling-p)
	   (not cl--not-toplevel) (not (boundp 'for-effect))) ;Horrible kludge.
      (let ((comp (or (memq 'compile when) (memq :compile-toplevel when)))
	    (cl--not-toplevel t))
	(if (or (memq 'load when) (memq :load-toplevel when))
	    (if comp (cons 'progn (mapcar #'cl--compile-time-too body))
	      `(if nil nil ,@body))
	  (progn (if comp (eval (cons 'progn body) lexical-binding)) nil)))
    (and (or (memq 'eval when) (memq :execute when))
	 (cons 'progn body))))

(defun cl--compile-time-too (form)
  (or (and (symbolp (car-safe form)) (get (car-safe form) 'byte-hunk-handler))
      (setq form (macroexpand
		  form (cons '(cl-eval-when) macroexpand-all-environment))))
  (cond ((eq (car-safe form) 'progn)
	 (cons 'progn (mapcar #'cl--compile-time-too (cdr form))))
	((eq (car-safe form) 'cl-eval-when)
	 (let ((when (nth 1 form)))
	   (if (or (memq 'eval when) (memq :execute when))
	       `(cl-eval-when (compile ,@when) ,@(cddr form))
	     form)))
	(t (eval form lexical-binding) form)))

(defmacro cl-load-time-value (form &optional _read-only)
  "Like `progn', but evaluates the body at load time.
The result of the body appears to the compiler as a quoted constant."
  (declare (debug (form &optional sexp)))
  (if (macroexp-compiling-p)
      (let* ((temp (cl-gentemp "--cl-load-time--"))
	     (set `(setq ,temp ,form)))
	(if (and (fboundp 'byte-compile-file-form-defmumble)
		 (boundp 'this-kind) (boundp 'that-one))
            ;; Else, we can't output right away, so we have to delay it to the
            ;; next time we're at the top-level.
            ;; FIXME: Use advice-add/remove.
            (fset 'byte-compile-file-form
                  (let ((old (symbol-function 'byte-compile-file-form)))
                    (lambda (form)
                      (fset 'byte-compile-file-form old)
                      (byte-compile-file-form set)
                      (byte-compile-file-form form))))
          ;; If we're not in the middle of compiling something, we can
          ;; output directly to byte-compile-outbuffer, to make sure
          ;; temp is set before we use it.
          (print set byte-compile--outbuffer))
	`(quote ,temp))
    (list 'quote (eval form lexical-binding))))

;;; Multiple values.

(defmacro cl-multiple-value-bind (vars form &rest body)
  "Collect multiple return values.
FORM must return a list; the BODY is then executed with the first N elements
of this list bound (`let'-style) to each of the symbols SYM in turn.  This is
analogous to the Common Lisp `multiple-value-bind' macro, using lists to
simulate true multiple return values.  For compatibility, (cl-values A B C) is
a synonym for (list A B C).

\(fn (SYM...) FORM BODY)"
  (declare (indent 2) (debug ((&rest symbolp) form body)))
  (let ((temp (make-symbol "--cl-var--")) (n -1))
    `(let* ((,temp ,form)
            ,@(mapcar (lambda (v)
                        (list v `(nth ,(setq n (1+ n)) ,temp)))
                      vars))
       ,@body)))

(defmacro cl-multiple-value-setq (vars form)
  "Collect multiple return values.
FORM must return a list; the first N elements of this list are stored in
each of the symbols SYM in turn.  This is analogous to the Common Lisp
`multiple-value-setq' macro, using lists to simulate true multiple return
values.  For compatibility, (cl-values A B C) is a synonym for (list A B C).

\(fn (SYM...) FORM)"
  (declare (indent 1) (debug ((&rest symbolp) form)))
  (cond ((null vars) `(progn ,form nil))
	((null (cdr vars)) `(setq ,(car vars) (car ,form)))
	(t
	 (let* ((temp (make-symbol "--cl-var--")) (n 0))
	   `(let ((,temp ,form))
              (prog1 (setq ,(pop vars) (car ,temp))
                (setq ,@(apply #'nconc
                               (mapcar (lambda (v)
                                         (list v `(nth ,(setq n (1+ n))
                                                       ,temp)))
                                       vars)))))))))

;;; Type declarations.

(defvar cl--optimize-safety)

(defmacro cl-the (type form)
  "Return FORM.  If type-checking is enabled, assert that it is of TYPE."
  (declare (indent 1) (debug (cl-type-spec form)))
  ;; When native compiling possibly add the appropriate type hint.
  (when (and (boundp 'byte-native-compiling)
             byte-native-compiling)
    (setf form
          (cl-case type
            (fixnum `(comp-hint-fixnum ,form))
            (cons `(comp-hint-cons ,form))
            (otherwise form))))
  (if (not (or (not (macroexp-compiling-p))
               (< cl--optimize-speed 3)
               (= cl--optimize-safety 3)))
      form
    (macroexp-let2 macroexp-copyable-p temp form
      `(progn (unless (cl-typep ,temp ',type)
                (signal 'wrong-type-argument
                        (list ',type ,temp ',form)))
              ,temp))))

;;; Declarations.

(defvar cl--proclaim-history t)    ; for future compilers
(defvar cl--declare-stack t)       ; for future compilers

(defun cl--do-proclaim (spec hist)
  (and hist (listp cl--proclaim-history) (push spec cl--proclaim-history))
  (cond ((eq (car-safe spec) 'special)
	 (if (boundp 'byte-compile-bound-variables)
	     (setq byte-compile-bound-variables
		   (append (cdr spec) byte-compile-bound-variables))))

	((eq (car-safe spec) 'inline)
	 (while (setq spec (cdr spec))
	   (or (memq (get (car spec) 'byte-optimizer)
		     '(nil byte-compile-inline-expand))
	       (error "%s already has a byte-optimizer, can't make it inline"
		      (car spec)))
	   (put (car spec) 'byte-optimizer #'byte-compile-inline-expand)))

	((eq (car-safe spec) 'notinline)
	 (while (setq spec (cdr spec))
	   (if (eq (get (car spec) 'byte-optimizer)
		   #'byte-compile-inline-expand)
	       (put (car spec) 'byte-optimizer nil))))

	((eq (car-safe spec) 'optimize)
	 (let ((speed (assq (nth 1 (assq 'speed (cdr spec)))
			    '((0 nil) (1 t) (2 t) (3 t))))
	       (safety (assq (nth 1 (assq 'safety (cdr spec)))
			     '((0 t) (1 nil) (2 nil) (3 nil)))))
	   (if speed (setq cl--optimize-speed (car speed)
			   byte-optimize (nth 1 speed)))
	   (if safety (setq cl--optimize-safety (car safety)
			    byte-compile-delete-errors (nth 1 safety)))))

	((and (eq (car-safe spec) 'warn) (boundp 'byte-compile-warnings))
	 (while (setq spec (cdr spec))
	   (if (consp (car spec))
               (if (eq (cadar spec) 0)
                   (byte-compile-disable-warning (caar spec))
                 (byte-compile-enable-warning (caar spec)))))))
  nil)

(defmacro cl-declare (&rest specs)
  "Declare SPECS about the current function while compiling.
For instance

  (cl-declare (warn 0))

will turn off byte-compile warnings in the function.
See Info node `(cl)Declarations' for details."
  (declare (obsolete defvar "31.1"))
  (if (macroexp-compiling-p)
      (while specs
	(if (listp cl--declare-stack) (push (car specs) cl--declare-stack))
	(cl--do-proclaim (pop specs) nil)))
  nil)

;;; Accessor shorthand.

(defmacro cl-with-accessors (bindings instance &rest body)
  "Use BINDINGS as function calls on INSTANCE inside BODY.

This macro helps when writing code that makes repeated use of the
accessor functions of a structure or object instance, such as those
created by `cl-defstruct' and `defclass'.

BINDINGS is a list of (NAME ACCESSOR) pairs.  Inside BODY, NAME is
treated as the function call (ACCESSOR INSTANCE) using
`cl-symbol-macrolet'.  NAME can be used with `setf' and `setq' as a
generalized variable.  Because of how the accessor is used,
`cl-with-accessors' can be used with any generalized variable that can
take a single argument, such as `car' and `cdr'.

See also the macro `with-slots' described in the Info
node `(eieio)Accessing Slots', which is similar, but uses slot names
instead of accessor functions.

\(fn ((NAME ACCESSOR) ...) INSTANCE &rest BODY)"
  (declare (debug [(&rest (symbolp symbolp)) form body])
           (indent 2))
  (cond ((null body)
         (macroexp-warn-and-return "`cl-with-accessors' used with empty body"
                                   nil 'empty-body))
        ((null bindings)
         (macroexp-warn-and-return "`cl-with-accessors' used without accessors"
                                   (macroexp-progn body)
                                   'suspicious))
        (t
         (cl-once-only (instance)
           (let ((symbol-macros))
             (dolist (b bindings)
               (pcase b
                 (`(,(and (pred symbolp) var)
                    ,(and (pred symbolp) accessor))
                  (push `(,var (,accessor ,instance))
                        symbol-macros))
                 (_
                  (error "Malformed `cl-with-accessors' binding: %S" b))))
             `(cl-symbol-macrolet
                  ,symbol-macros
                ,@body))))))

(provide 'cl-macs)
;;; cl-macs.el ends here
