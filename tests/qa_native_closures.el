;;; -*- lexical-binding: t; -*-
;; Issue #184: closures created inside a native function capture its cells.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defvar qa184-directory (make-temp-file "remacs-qa-native-closures-" t))
(defvar qa184-serial 0)
(defvar qa184-factory nil)
(defvar qa184-first nil)
(defvar qa184-second nil)
(defvar qa184-special 'global)
(defvar qa184-side nil)
(setq qa184-free 'global)
(defun qa184-named (x) (lambda () x))
(defmacro qa184-capture (value) `(lambda () ,value))
(defun qa184-compile (form)
  (setq qa184-serial (1+ qa184-serial))
  (native-compile
   (eval form t)
   (expand-file-name (format "case%d.eln" qa184-serial) qa184-directory)))

(unwind-protect
    (let ((user-emacs-directory qa184-directory)
          (native-comp-eln-load-path (list qa184-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_closures requires native compilation"))
      ;; Original reproduction, including a call after the factory returns.
      (setq qa184-factory (qa184-compile '(lambda (x) (lambda () x))))
      (qa (native-comp-function-p qa184-factory) t)
      (qa (functionp (funcall qa184-factory 7)) t)
      (qa (funcall (funcall qa184-factory 7)) 7)
      (qa (funcall (funcall qa184-factory nil)) nil)
      (qa (funcall (funcall qa184-factory 'value)) 'value)
      (qa (let ((object (list 1 2)))
            (eq (funcall (funcall qa184-factory object)) object)) t)
      (setq qa184-first (funcall qa184-factory 7)
            qa184-second (funcall qa184-factory 9))
      (qa (list (funcall qa184-first) (funcall qa184-second)
                (funcall qa184-first)) '(7 9 7))
      ;; Capture the factory's argument, never a same-named caller binding.
      (qa-lex (let ((x 'caller)) (funcall (funcall qa184-factory 11))) 11)
      (qa (funcall (funcall (qa184-compile '(lambda (x) #'(lambda () x))) 12))
          12)
      (qa (funcall (funcall (qa184-compile '(lambda (x) (qa184-capture x))) 13))
          13)

      ;; Optional and rest arguments are still live after return.
      (setq qa184-factory
            (qa184-compile '(lambda (x &optional y &rest z)
                             (lambda () (list x y z)))))
      (qa (funcall (funcall qa184-factory 1)) '(1 nil nil))
      (qa (funcall (funcall qa184-factory 1 2 3 4)) '(1 2 (3 4)))
      (qa (funcall (funcall (qa184-compile
                            '(lambda (x) (lambda (y) (+ x y)))) 4) 5) 9)
      (qa (funcall (funcall (funcall (qa184-compile
                                     '(lambda (x)
                                        (lambda (y) (lambda () (+ x y)))))
                                    4) 6)) 10)

      ;; Let/let* scopes and shadowed names are lexical cells too.
      (qa (funcall (funcall (qa184-compile
                            '(lambda (x) (let ((y (+ x 1)))
                                           (lambda () (list x y))))) 3))
          '(3 4))
      (qa (funcall (funcall (qa184-compile
                            '(lambda (x) (let* ((x (+ x 1)) (y (* x 2)))
                                           (lambda () (list x y))))) 3))
          '(4 8))
      (qa (funcall (funcall (qa184-compile
                            '(lambda (x) (let ((x 'inner)) (lambda () x))))
                           'outer)) 'inner)
      (qa (funcall (qa184-compile
                    '(lambda (x) (funcall (lambda (y) (+ x y)) 2))) 5) 7)
      (qa (funcall (qa184-compile
                    '(lambda (x) (mapcar (lambda (y) (+ x y)) '(1 2 3)))) 10)
          '(11 12 13))

      ;; Mutations are shared by all closures from the same invocation,
      ;; including writes performed by the enclosing function after creation.
      (setq qa184-factory
            (qa184-compile
             '(lambda (x)
                (list (lambda () x) (lambda (value) (setq x value))))))
      (setq qa184-first (funcall qa184-factory 3)
            qa184-second (funcall qa184-factory 8))
      (qa (funcall (car qa184-first)) 3)
      (qa (funcall (car (cdr qa184-first)) 6) 6)
      (qa (funcall (car qa184-first)) 6)
      (qa (funcall (car qa184-second)) 8)
      (qa (funcall (funcall (qa184-compile
                            '(lambda (x) (let ((read (lambda () x)))
                                           (setq x (+ x 1)) read))) 20)) 21)
      (setq qa184-first
            (funcall (qa184-compile
                      '(lambda (x) (lambda () (setq x (+ x 1))))) 0))
      (qa (list (funcall qa184-first) (funcall qa184-first)) '(1 2))

      ;; Branches, side effects, and free/special variables retain semantics.
      (setq qa184-factory
            (qa184-compile '(lambda (x) (if x (lambda () x) (lambda () 'no)))))
      (qa (funcall (funcall qa184-factory 'yes)) 'yes)
      (qa (funcall (funcall qa184-factory nil)) 'no)
      (setq qa184-factory
            (qa184-compile '(lambda () (lambda () qa184-free))))
      (qa-lex (let ((qa184-free 'caller)) (funcall (funcall qa184-factory)))
          'global)
      (setq qa184-free 'updated)
      (qa (funcall (funcall qa184-factory)) 'updated)
      (setq qa184-first
            (funcall (qa184-compile
                      '(lambda () (lambda () qa184-special)))))
      (qa (funcall qa184-first) 'global)
      (qa (let ((qa184-special 'dynamic)) (funcall qa184-first)) 'dynamic)
      (setq qa184-side nil)
      (qa-err (funcall (qa184-compile
                        '(lambda () (error "stop")
                           (setq qa184-side t) (lambda () 1)))) 'error)
      (qa qa184-side nil)
      (qa (funcall (funcall (qa184-compile '(lambda () (lambda () 42))))) 42)
      (qa (funcall (qa184-compile '(lambda () (list #'car '(lambda () x)))))
          '(car (lambda () x)))

      ;; A dynamic factory must not gain a lexical environment in fallback.
      (setq qa184-factory
            (native-compile
             (eval '(lambda (qa184-dynamic) (lambda () qa184-dynamic)) nil)
             (expand-file-name "dynamic.eln" qa184-directory)))
      (qa-err-data (funcall (funcall qa184-factory 7))
          'void-variable '(qa184-dynamic))
      (qa (let ((qa184-dynamic 'caller))
            (funcall (funcall qa184-factory 7))) 'caller)

      ;; Symbol and file compilation use the same closure-safe emission.
      (setq qa184-factory
            (native-compile 'qa184-named
                            (expand-file-name "named.eln" qa184-directory)))
      (qa (native-comp-function-p qa184-factory) t)
      (qa (funcall (funcall qa184-factory 31)) 31)
      (let ((source (expand-file-name "closure.el" qa184-directory))
            (output (expand-file-name "closure.eln" qa184-directory)))
        (with-temp-file source
          (insert ";;; -*- lexical-binding: t; -*-\n"
                  "(defun qa184-file-factory (x) (lambda () x))\n"
                  "(defun qa184-file-global (x) "
                  "(lambda () (list x qa184-free)))\n"
                  "(defun qa184-file-missing (x) "
                  "(lambda () (list x qa184-missing)))\n"))
        (native-compile source output)
        (load output nil t)
        (qa (native-comp-function-p (symbol-function 'qa184-file-factory)) t)
        (qa (funcall (qa184-file-factory 41)) 41)
        (qa (funcall (qa184-file-global 42)) '(42 updated))
        (qa-err-data (funcall (qa184-file-missing 43))
            'void-variable '(qa184-missing)))
      (let ((source (expand-file-name "dynamic.el" qa184-directory))
            (output (expand-file-name "dynamic-file.eln" qa184-directory)))
        (with-temp-file source
          (insert ";;; -*- lexical-binding: nil; -*-\n"
                  "(defun qa184-file-dynamic (qa184-dynamic) "
                  "(lambda () qa184-dynamic))\n"))
        (native-compile source output)
        (load output nil t)
        (qa (native-comp-function-p (symbol-function 'qa184-file-dynamic)) t)
        (qa-err-data (funcall (qa184-file-dynamic 41))
            'void-variable '(qa184-dynamic))))
  (delete-directory qa184-directory t))
(princ (qa--done)) (terpri)
