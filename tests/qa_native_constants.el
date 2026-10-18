;;; -*- lexical-binding: t; -*-
;; Issue #189: a native function retains its literal objects across calls.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defmacro qa189-check (form expected)
  `(let ((actual (condition-case err ,form
                   (error (list 'qa--err (car err) (cdr err)))))
         (wanted ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal actual wanted)
       (push (list ',form wanted actual) qa--fails))))
(defvar qa189-directory (make-temp-file "remacs-qa-native-constants-" t))
(defvar qa189-serial 0)
(defvar qa189-loaded nil)
(defun qa189-compile (fn)
  (setq qa189-serial (1+ qa189-serial))
  (native-compile fn (expand-file-name (format "lambda%d.eln" qa189-serial)
                                      qa189-directory)))

(unwind-protect
    (let ((user-emacs-directory qa189-directory)
          (native-comp-eln-load-path (list qa189-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_constants requires native compilation"))
      ;; The issue reproduction and adjacent quoted/self-evaluating objects.
      (dolist (form '((lambda () '(1 2))
                      (lambda () '(1 . 2))
                      (lambda () '((1 2) [3 4] "five"))
                      (lambda () '[1 2])
                      (lambda () [1 2])
                      (lambda () '"literal")
                      (lambda () (progn "literal"))
                      (lambda () '1.25)
                      (lambda () 1.25)
                      (lambda () '#&3"\7")
                      (lambda () '#s(qa189-record 1 2))
                      (lambda () '#:qa189-uninterned)
                      (lambda () nil)
                      (lambda () t)
                      (lambda () 'symbol)))
        (let* ((fn (qa189-compile (eval form t)))
               (first (funcall fn)))
          (qa189-check (native-comp-function-p fn) t)
          (let ((expected (funcall (eval form t))))
            (qa189-check (if (symbolp expected)
                             (equal (symbol-name first) (symbol-name expected))
                           (equal first expected)) t))
          (qa189-check (eq first (funcall fn)) t)
          (garbage-collect)
          (qa189-check (eq first (funcall fn)) t)))

      ;; Direct strings retain their raw character representation. Printing a
      ;; surrogate as Lisp source is lossy, even though raw construction works.
      (dolist (characters '((55296) (56320) (0 34 92 10 233 55296 56320)))
        (let* ((literal (apply #'string characters))
               (fn (qa189-compile (eval (list 'lambda nil literal) t))))
          (qa189-check (native-comp-function-p fn) t)
          (qa189-check (string-to-list (funcall fn)) characters)
          (qa189-check (eq (funcall fn) (funcall fn)) t)))
      ;; A C compiler can merge identical byte literals used for a raw string
      ;; and for read syntax. Their constant-cache keys must remain distinct.
      (let* ((fn (qa189-compile
                  (lambda () (list "(1 2)" '(1 2) "1.25" 1.25))))
             (first (funcall fn))
             (second (funcall fn)))
        (qa189-check (native-comp-function-p fn) t)
        (qa189-check first '("(1 2)" (1 2) "1.25" 1.25))
        (qa189-check (eq first second) nil)
        (dotimes (index 4)
          (qa189-check (eq (nth index first) (nth index second)) t)))

      ;; Unsupported control forms and captured local factories use fallback,
      ;; which must retain the function's body literals too.
      (dolist (form '((lambda () (unwind-protect '(1 2) nil))
                      (lambda () (condition-case nil '(3 4) (error nil)))
                      (lambda () (catch 'tag '(5 6)))
                      (lambda () (save-excursion '(7 8)))
                      (lambda () (funcall (lambda () '(9 10))))))
        (let ((fn (qa189-compile (eval form t))))
          (qa189-check (native-comp-function-p fn) t)
          (qa189-check (eq (funcall fn) (funcall fn)) t)))
      (let* ((factory (qa189-compile (lambda (x) (lambda () (cons x '(tail))))))
             (first (funcall factory 1))
             (second (funcall factory 2)))
        (qa189-check (native-comp-function-p factory) t)
        (qa189-check (funcall first) '(1 tail))
        (qa189-check (funcall second) '(2 tail))
        (qa189-check (eq (cdr (funcall first)) (cdr (funcall first))) t)
        (qa189-check (eq (cdr (funcall first)) (cdr (funcall second))) t))

      ;; Ordinary runtime allocation remains fresh even though its arguments
      ;; can be constants. Every value/identity check runs through native code.
      (dolist (form '((lambda () (list 1 2))
                      (lambda () (cons 1 2))
                      (lambda () (vector 1 2))
                      (lambda () (copy-sequence '(1 2)))
                      (lambda () (copy-sequence "fresh"))))
        (let ((fn (qa189-compile (eval form t))))
          (qa189-check (native-comp-function-p fn) t)
          (qa189-check (equal (funcall fn) (funcall fn)) t)
          (qa189-check (eq (funcall fn) (funcall fn)) nil)))

      ;; Nested calls, recursion and signals must restore the active cache;
      ;; no cached entry may be a handle discarded at the end of a call.
      (let* ((inner (qa189-compile (lambda () '(inner))))
             (outer (qa189-compile
                     (lambda (fn fail)
                       (let ((constant '(outer)))
                         (funcall fn)
                         (if fail (error "stop"))
                         constant))))
             (first (funcall outer inner nil)))
        (qa189-check first '(outer))
        (qa189-check (funcall inner) '(inner))
        (qa189-check (eq first (funcall outer inner nil)) t)
        (qa189-check (condition-case nil (funcall outer inner t)
                       (error 'caught)) 'caught)
        (qa189-check (eq first (funcall outer inner nil)) t))
      (fset 'qa189-recursive
            (lambda (n) (let ((constant '(recursive)))
                          (if (> n 0) (qa189-recursive (1- n)))
                          constant)))
      (let* ((fn (qa189-compile 'qa189-recursive))
             (first (funcall fn 4)))
        (qa189-check (native-comp-function-p fn) t)
        (qa189-check first '(recursive))
        (qa189-check (eq first (funcall fn 0)) t)
        (qa189-check (eq first (funcall fn 3)) t))

      ;; File compilation, source binding modes, load-time calls and reloads.
      ;; Previously obtained functions keep their objects after registration
      ;; of newer functions from the same shared library.
      (dolist (dynamic '(nil t))
        (let* ((source (expand-file-name (if dynamic "dynamic.el" "lexical.el")
                                        qa189-directory))
               (output (concat source "n")))
          (with-temp-file source
            (insert (if dynamic ";;; -*- lexical-binding: nil; -*-\n"
                      ";;; -*- lexical-binding: t; -*-\n")
                    "(defun qa189-file () '(file constant))\n"
                    "(defun qa189-file-fallback () (unwind-protect '(fallback constant) nil))\n"
                    "(setq qa189-loaded (eq (qa189-file) (qa189-file)))\n"))
          (setq qa189-loaded 'before-load)
          (native-compile source output)
          (qa189-check qa189-loaded 'before-load)
          (native-elisp-load output)
          (qa189-check qa189-loaded t)
          (qa189-check (native-comp-function-p (symbol-function 'qa189-file)) t)
          (qa189-check (eq (qa189-file) (qa189-file)) t)
          (qa189-check (eq (qa189-file-fallback) (qa189-file-fallback)) t)
          (let* ((old (symbol-function 'qa189-file))
                 (first (funcall old)))
            (native-elisp-load output)
            (qa189-check (eq first (funcall old)) t)
            (qa189-check (eq (qa189-file) (qa189-file)) t)))))
  (delete-directory qa189-directory t))

(princ (concat (qa--done) "\n"))
