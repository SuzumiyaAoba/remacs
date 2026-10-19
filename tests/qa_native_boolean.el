;;; -*- lexical-binding: t; -*-
;; Issue #188: native conjunction has t as its zero-operand identity.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defmacro qa188-check (form expected)
  `(let ((actual (condition-case err ,form
                   (error (list 'qa--err (car err) (cdr err)))))
         (wanted ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal actual wanted)
       (push (list ',form wanted actual) qa--fails))))
(defvar qa188-directory (make-temp-file "remacs-qa-native-boolean-" t))
(defvar qa188-serial 0)
(defvar qa188-loaded nil)
(defun qa188-compile (fn)
  (setq qa188-serial (1+ qa188-serial))
  (native-compile fn (expand-file-name (format "lambda%d.eln" qa188-serial)
                                      qa188-directory)))

(unwind-protect
    (let ((user-emacs-directory qa188-directory)
          (native-comp-eln-load-path (list qa188-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_boolean requires native compilation"))
      ;; Lambda compilation: empty, singleton and nested identities, and
      ;; non-boolean values. Every returned function must really be native.
      (dolist (case '(((lambda () (and)) t)
                      ((lambda () (or)) nil)
                      ((lambda () (and t)) t)
                      ((lambda () (and nil)) nil)
                      ((lambda () (and 0)) 0)
                      ((lambda () (and "")) "")
                      ((lambda () (and 'value)) value)
                      ((lambda () (or 'value)) value)
                      ((lambda () (and (and))) t)
                      ((lambda () (and (and) (and))) t)
                      ((lambda () (and 17 (and))) t)
                      ((lambda () (and (and) 17)) 17)
                      ((lambda () (or (and) 17)) t)
                      ((lambda () (or (or) (and))) t)
                      ((lambda () (and (or))) nil)
                      ((lambda () (and (and) (or))) nil)
                      ((lambda () (if (and) 'yes 'no)) yes)
                      ((lambda () (and (or nil 2) (and 3 (and)) 4)) 4)
                      ((lambda () (and nil (error "must not run"))) nil)
                      ((lambda () (or (and) (error "must not run"))) t)))
        (let ((fn (qa188-compile (eval (car case) t))))
          (qa188-check (native-comp-function-p fn) t)
          (qa188-check (funcall fn) (cadr case))))

      ;; Nonempty conjunction/disjunction still return the exact operand,
      ;; including mutable objects, and accept nil in each argument position.
      (let ((conjunction (qa188-compile (lambda (a b c) (and a b c))))
            (disjunction (qa188-compile (lambda (a b c) (or a b c)))))
        (qa188-check (native-comp-function-p conjunction) t)
        (qa188-check (native-comp-function-p disjunction) t)
        (dolist (a '(nil 1))
          (dolist (b '(nil 2))
            (dolist (c '(nil 3))
              (qa188-check (funcall conjunction a b c) (and a b c))
              (qa188-check (funcall disjunction a b c) (or a b c)))))
        (let ((object (list 'mutable)))
          (qa188-check (eq (funcall conjunction t 0 object) object) t)
          (qa188-check (eq (funcall disjunction nil object t) object) t)))

      ;; Each visited operand runs exactly once, in source order. Nested
      ;; empty forms do not skip needed operands or run short-circuited ones.
      (let ((fn (qa188-compile
                 (lambda (a b)
                   (let* ((seen nil)
                          (result (and (and)
                                       (progn (setq seen (cons 'first seen)) a)
                                       (progn (setq seen (cons 'second seen)) b)
                                       (progn (setq seen (cons 'third seen)) (and)))))
                     (list result (reverse seen)))))))
        (qa188-check (native-comp-function-p fn) t)
        (qa188-check (funcall fn nil t) '(nil (first)))
        (qa188-check (funcall fn t nil) '(nil (first second)))
        (qa188-check (funcall fn 1 2) '(t (first second third))))
      (let ((fn (qa188-compile
                 (lambda (a)
                   (let* ((seen nil)
                          (result (or (progn (setq seen (cons 'first seen)) a)
                                      (and)
                                      (progn (setq seen (cons 'last seen)) 9))))
                     (list result (reverse seen)))))))
        (qa188-check (native-comp-function-p fn) t)
        (qa188-check (funcall fn nil) '(t (first)))
        (qa188-check (funcall fn 5) '(5 (first))))

      ;; Compiling a named function follows the same native path.
      (fset 'qa188-symbol (lambda () (and)))
      (let ((fn (qa188-compile 'qa188-symbol)))
        (qa188-check (native-comp-function-p fn) t)
        (qa188-check (funcall fn) t)
        (qa188-check (qa188-symbol) t))

      ;; File compilation covers lexical/dynamic files, generated top-level
      ;; initialization and reloading the emitted unit.
      (dolist (dynamic '(nil t))
        (let* ((source (expand-file-name (if dynamic "dynamic.el" "lexical.el")
                                        qa188-directory))
               (output (concat source "n")))
          (with-temp-file source
            (insert (if dynamic ";;; -*- lexical-binding: nil; -*-\n"
                      ";;; -*- lexical-binding: t; -*-\n")
                    "(defun qa188-file-and () (and))\n"
                    "(defun qa188-file-or () (or))\n"
                    "(defun qa188-file-nested (x) (and (and) x (and)))\n"
                    "(setq qa188-loaded (list (and) (or)))\n"))
          (setq qa188-loaded 'before-load)
          (native-compile source output)
          (qa188-check (file-exists-p output) t)
          (qa188-check qa188-loaded 'before-load)
          (native-elisp-load output)
          (qa188-check qa188-loaded '(t nil))
          (qa188-check (native-comp-function-p (symbol-function 'qa188-file-and)) t)
          (qa188-check (native-comp-function-p (symbol-function 'qa188-file-or)) t)
          (qa188-check (native-comp-function-p (symbol-function 'qa188-file-nested)) t)
          (qa188-check (qa188-file-and) t)
          (qa188-check (qa188-file-or) nil)
          (qa188-check (qa188-file-nested nil) nil)
          (qa188-check (qa188-file-nested 7) t)
          (setq qa188-loaded 'reload)
          (native-elisp-load output)
          (qa188-check qa188-loaded '(t nil)))))
  (delete-directory qa188-directory t))

(princ (concat (qa--done) "\n"))
