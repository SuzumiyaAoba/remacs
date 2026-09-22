;;; let-alist.el --- Easily let-bind values of an assoc-list by their names -*- lexical-binding: t -*-

;; Ported from GNU Emacs let-alist.el (Artur Malabarba).
;; The `let-alist' macro takes an alist expression and a body, and
;; let-binds every dotted symbol in BODY to its cdr in the alist.
;; `.foo.0' indexes the value's list; `..foo' refers to a `.foo'
;; variable bound outside the `let-alist'.

;;; Code:

(defun let-alist--deep-dot-search (data)
  "Return alist of symbols inside DATA that start with a `.'.
Perform a deep search and return an alist where each car is the
symbol, and each cdr is the same symbol without the `.'."
  (cond
   ((symbolp data)
    (let ((name (symbol-name data)))
      (when (string-match "\\`\\." name)
        ;; Return the cons cell inside a list, so it can be appended
        ;; with other results in the clause below.
        (list (cons data (intern (replace-match "" nil nil name)))))))
   ((vectorp data)
    (apply #'nconc (mapcar #'let-alist--deep-dot-search data)))
   ((not (consp data)) nil)
   ((eq (car data) 'let-alist)
    ;; For nested ‘let-alist’ forms, ignore symbols appearing in the
    ;; inner body because they don’t refer to the alist currently
    ;; being processed.  See Bug#24641.
    (let-alist--deep-dot-search (cadr data)))
   (t (append (let-alist--deep-dot-search (car data))
              (let-alist--deep-dot-search (cdr data))))))

(defun let-alist--access-sexp (symbol variable)
  "Return a sexp used to access SYMBOL inside VARIABLE."
  (let* ((clean (let-alist--remove-dot symbol))
         (name (symbol-name clean)))
    (if (string-match "\\`\\." name)
        clean
      (let-alist--list-to-sexp
       (mapcar #'read (nreverse (split-string name "\\.")))
       variable))))

(defun let-alist--list-to-sexp (list var)
  "Turn symbols LIST into recursive calls to `cdr' `assq' on VAR."
  (let ((sym (car list))
        (rest (if (cdr list) (let-alist--list-to-sexp (cdr list) var)
                 var)))
    (cond
     ((numberp sym) `(nth ,sym ,rest))
     (t `(cdr (assq ',sym ,rest))))))

(defun let-alist--remove-dot (symbol)
  "Return SYMBOL, sans an initial dot."
  (let ((name (symbol-name symbol)))
    (if (string-match "\\`\\." name)
        (intern (replace-match "" nil nil name))
      symbol)))


;;; The actual macro.
;;;###autoload
(defmacro let-alist (alist &rest body)
  "Let-bind dotted symbols to their cdrs in ALIST and execute BODY.
Dotted symbol is any symbol starting with a `.'.  This macro creates
let-bindings for dotted symbols that appear literally in BODY (whether
or not they are actually used).  It does not create bindings for dotted
symbols that are introduced by macro-expansion in BODY.

A symbol of the form `.foo.N' where N is a natural number refers to the
Nth element of the value that ALIST associates to key `foo'.

For instance, the following code

  (let-alist alist
    (if (and .title.0 .body)
        .body
      .site
      .site.contents))

essentially expands to

  (let ((.title.0 (nth 0 (cdr (assq \\='title alist))))
        (.body  (cdr (assq \\='body alist)))
        (.site  (cdr (assq \\='site alist)))
        (.site.contents (cdr (assq \\='contents (cdr (assq \\='site alist))))))
    (if (and .title.0 .body)
        .body
      .site
      .site.contents))

If you nest `let-alist' invocations, the inner one can't access
the variables of the outer one.  You can, however, access alists
inside the original alist by using dots inside the symbol, as
displayed in the example above.

To refer to a non-`let-alist' variable starting with a dot in BODY, use
two dots instead of one.  For example, in the following form `..foo'
refers to the variable `.foo' bound outside of the `let-alist':

    (let ((.foo 42)) (let-alist \\='((foo . nil)) ..foo))

Note that there is no way to differentiate the case where a key
is missing from when it is present, but its value is nil.  Thus,
the following form evaluates to nil:

    (let-alist \\='((some-key . nil))
      .some-key)"
  (declare (indent 1) (debug t))
  (let ((var (make-symbol "alist")))
    `(let ((,var ,alist))
       (let ,(mapcar (lambda (x) `(,(car x) ,(let-alist--access-sexp (car x) var)))
                     (delete-dups (let-alist--deep-dot-search body)))
         ,@body))))

(provide 'let-alist)

;;; let-alist.el ends here
