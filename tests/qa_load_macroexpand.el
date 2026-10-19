;;; -*- lexical-binding: nil; -*-
;; Issue #10: GNU readevalloop performs head and full eager expansion.
(defvar qa10-result nil)
(defvar qa10-count 0)
(defvar qa10-expansions nil)
(defvar qa10-file nil)
(defvar qa10-calls nil)
(defvar qa10-autoload-hit nil)

(defun qa10-load (source &optional lexical suffix)
  (let ((qa10-file (make-temp-file "remacs-eager-" nil (or suffix ".el"))))
    (unwind-protect
        (progn
          (with-temp-file qa10-file
            (insert (if (equal suffix ".elc")
                        ";ELC\x1f\0\0\0\n;;; in Emacs version 31.1\n"
                      (if lexical
                          ";;; -*- lexical-binding: t; -*-\n"
                        ";;; -*- lexical-binding: nil; -*-\n")))
            (insert source))
          (load qa10-file nil t t))
      (delete-file qa10-file))))

;; Compiler macros inside lexical scopes must run before evaluation.
(qa (progn
      (qa10-load "(setq qa10-result (let ((qa10-local '(1 2))) (add-to-list 'qa10-local 0) qa10-local))" t)
      qa10-result)
    '(0 1 2))
(qa (progn
      (qa10-load "(setq qa10-result (let ((qa10-local '(1 2))) (add-to-list 'qa10-local 3 t) qa10-local))" t)
      qa10-result)
    '(1 2 3))
(qa (progn
      (qa10-load "(setq qa10-result (let ((qa10-local '(1))) (funcall (lambda () (add-to-list 'qa10-local 0))) qa10-local))" t)
      qa10-result)
    '(0 1))
(qa-err (eval '(let ((qa10-local '(1))) (add-to-list 'qa10-local 0) qa10-local) t)
        'void-variable)

;; Bodies freeze the macro definition that was active while loading.
(qa (progn
      (qa10-load "(progn
 (defmacro qa10-frozen-macro () ''old)
 (defun qa10-frozen-function () (qa10-frozen-macro))
 (defmacro qa10-frozen-macro () ''new)
 (setq qa10-result (qa10-frozen-function)))" t)
      qa10-result)
    'old)
(qa (qa10-frozen-function) 'old)
(qa (progn
      (qa10-load "(defmacro qa10-splice () '(progn (defmacro qa10-generated () 7) (setq qa10-result (qa10-generated))))
(qa10-splice)" t)
      qa10-result)
    7)

;; All branches expand, including the branch that will never execute.
(defun qa10-compiler-function (arg) (list 'runtime arg))
(defun qa10-compiler-handler (_form arg)
  (push arg qa10-expansions)
  (list 'quote (list 'compiled arg)))
(put 'qa10-compiler-function 'compiler-macro #'qa10-compiler-handler)
(setq qa10-expansions nil)
(qa (progn
      (qa10-load "(setq qa10-result (list (qa10-compiler-function 1) (if nil (qa10-compiler-function 2) (qa10-compiler-function 3))))" t)
      qa10-result)
    '((compiled 1) (compiled 3)))
(qa (reverse qa10-expansions) '(1 2 3))
;; Shared reader objects still represent separate expansion occurrences.
(setq qa10-count 0)
(qa (progn
      (qa10-load "(defmacro qa10-counted () (setq qa10-count (1+ qa10-count)))
(setq qa10-result (list #1=(qa10-counted) #1#))" t)
      qa10-result)
    '(1 2))
(setq qa10-expansions nil)
(let ((macroexp-inhibit-compiler-macros t))
  (qa (progn
        (qa10-load "(setq qa10-result (qa10-compiler-function 4))" t)
        qa10-result)
      '(runtime 4)))
(qa qa10-expansions nil)
(defun qa10-nil-compiler () 'runtime)
(put 'qa10-nil-compiler 'compiler-macro (lambda (_form) nil))
(qa (progn (qa10-load "(setq qa10-result (qa10-nil-compiler))" t) qa10-result) nil)
(defun qa10-retry-compiler (_arg) 'runtime)
(put 'qa10-retry-compiler 'compiler-macro
     (lambda (form arg)
       (if (numberp arg) (list 'quote (list 'compiled arg)) form)))
(defmacro qa10-retry-argument () 5)
(qa (progn
      (qa10-load "(setq qa10-result (qa10-retry-compiler (qa10-retry-argument)))" t)
      qa10-result)
    '(compiled 5))
(defun qa10-exiting-compiler () 'runtime)
(put 'qa10-exiting-compiler 'compiler-macro
     (lambda (_form) (throw 'qa10-escape 'escaped)))
(qa (catch 'qa10-escape (qa10-load "(qa10-exiting-compiler)" t) 'after) 'escaped)
(qa (let ((saved (get 'quote 'compiler-macro)))
      (unwind-protect
          (progn
            (put 'quote 'compiler-macro (lambda (_form &rest _args) 99))
            (qa10-load "(setq qa10-result 'literal)" t)
            qa10-result)
        (put 'quote 'compiler-macro saved)))
    'literal)

;; Quoted forms stay data, while eval-when-compile runs during the load.
(setq qa10-count 0)
(qa (progn
      (qa10-load "(defmacro qa10-side-effect () (setq qa10-count (1+ qa10-count)) 8)
(setq qa10-result '(qa10-side-effect))" t)
      (list qa10-result qa10-count))
    '((qa10-side-effect) 0))
(setq qa10-count 0)
(qa (progn
      (qa10-load "(defun qa10-compile-time () (eval-when-compile (setq qa10-count (1+ qa10-count)) 'constant))" t)
      (list qa10-count (qa10-compile-time) (qa10-compile-time) qa10-count))
    '(1 constant constant 1))

;; A malformed macro in an uncalled body fails at load time.
(fmakunbound 'qa10-invalid-function)
(qa (condition-case err
        (qa10-load "(defun qa10-invalid-function () (when))" t)
      (error (string-prefix-p "Eager macro-expansion failure:"
                              (error-message-string err))))
    t)
(qa (fboundp 'qa10-invalid-function) nil)

;; Macro autoloads in function bodies resolve eagerly and keep their result.
(let ((macro-file (make-temp-file "remacs-eager-macro-" nil ".el")))
  (unwind-protect
      (progn
        (with-temp-file macro-file
          (insert ";;; -*- lexical-binding: t; -*-\n"
                  "(setq qa10-autoload-hit t)\n"
                  "(defmacro qa10-autoloaded () ''autoload-old)\n"))
        (setq qa10-autoload-hit nil)
        (autoload 'qa10-autoloaded macro-file nil nil t)
        (qa10-load "(defun qa10-autoload-user () (qa10-autoloaded))" t)
        (qa qa10-autoload-hit t)
        (defmacro qa10-autoloaded () ''autoload-new)
        (qa (qa10-autoload-user) 'autoload-old))
    (delete-file macro-file)))

;; The real hook sees both passes and the top-level progn before splicing.
(let ((saved (symbol-function 'internal-macroexpand-for-load)))
  (unwind-protect
      (progn
        (setq qa10-calls nil)
        (fset 'internal-macroexpand-for-load
              (lambda (form full)
                (when (equal load-file-name qa10-file)
                  (push (list (car-safe form) full) qa10-calls))
                form))
        (qa10-load "(progn 'one 'two)" t)
        (qa (reverse qa10-calls)
            '((progn nil) (quote nil) (quote t) (quote nil) (quote t)))
        (setq qa10-calls nil)
        (qa10-load "(setq qa10-result 'compiled)" nil ".elc")
        (qa qa10-result 'compiled)
        (qa qa10-calls nil))
    (fset 'internal-macroexpand-for-load saved)))

;; Availability is sampled once: no hook means ordinary interpreted eval.
(let ((saved (symbol-function 'internal-macroexpand-for-load)))
  (unwind-protect
      (progn
        (fmakunbound 'internal-macroexpand-for-load)
        (qa-err (qa10-load "(let ((qa10-local nil)) (add-to-list 'qa10-local 1))" t)
                'void-variable))
    (fset 'internal-macroexpand-for-load saved)))

;; File declarations do not leak into the caller's macroexp dynamic list.
(let ((macroexp--dynvars '(qa10-outer)))
  (qa10-load "(defvar qa10-file-declaration)" t)
  (qa macroexp--dynvars '(qa10-outer)))
(defun qa10-declaration-probe () 'runtime)
(put 'qa10-declaration-probe 'compiler-macro
     (lambda (_form)
       (list 'quote (and (memq 'qa10-top-declared macroexp--dynvars) t))))
(qa (progn
      (qa10-load "(defvar qa10-top-declared)
(setq qa10-result (qa10-declaration-probe))" t)
      qa10-result)
    t)
(qa (progn
      (qa10-load "(defun qa10-scoped-declaration () (defvar qa10-top-declared))
(setq qa10-result (qa10-declaration-probe))" t)
      qa10-result)
    nil)
(let ((macroexp--dynvars '(qa10-outer)))
  (macroexpand-all '(defvar qa10-explicit-declaration))
  (qa macroexp--dynvars '(qa10-outer)))
(princ (qa--done))
