;;; Native dynamic bindings retain compiled evaluation rules. -*- lexical-binding: t; -*-
(require 'comp)
(defvar qa-nb-directory (make-temp-file "qa-native-bindings-" t))
(defvar qa-nb-serial 0)
(defvar qa-nb-x 'outer)
(defvar qa-nb-y 'second)
(defvar qa-nb-side nil)
(defvar qa-nb-events nil)
(defvar qa-nb-function nil)
(defvar qa-nb-reader nil)
(setq qa-nb-free 'outside)
(defvaralias 'qa-nb-alias 'qa-nb-x)
(defun qa-nb-observe () (list qa-nb-x qa-nb-y))
(defun qa-nb-read-free () qa-nb-free)
(defun qa-nb-compile (body &optional arguments dynamic)
  (setq qa-nb-serial (1+ qa-nb-serial))
  (native-compile (eval (cons 'lambda (cons arguments body)) (not dynamic))
                  (expand-file-name (format "case-%d.eln" qa-nb-serial) qa-nb-directory)))
(defun qa-nb-run (body &optional arguments values dynamic)
  (apply (qa-nb-compile body arguments dynamic) values))
(defun qa-nb-deferred (function)
  (let ((inhibit-quit t) (quit-flag nil))
    (let ((result (condition-case nil
                      (let ((inhibit-quit t)) (funcall function))
                    (quit 'caught))))
      (prog1 (list result quit-flag) (setq quit-flag nil)))))

(unwind-protect
    (let ((user-emacs-directory qa-nb-directory)
          (native-comp-eln-load-path (list qa-nb-directory)))
      (qa (qa-nb-run '((let ((qa-nb-x 'inner) (local qa-nb-x)) (list qa-nb-x local)))) '(inner outer))
      (qa qa-nb-x 'outer)
      (qa (qa-nb-run '((let* ((qa-nb-x 'inner) (qa-nb-y qa-nb-x) (local qa-nb-y))
                         (list qa-nb-x qa-nb-y local)))) '(inner inner inner))
      (qa (qa-nb-observe) '(outer second))
      (qa (qa-nb-run '((let ((qa-nb-x 'inner)) (list qa-nb-x (qa-nb-observe))))
                      '(qa-nb-x) '(argument)) '(inner (inner second)))
      (qa (qa-nb-run '((let* ((qa-nb-x 'inner) (local qa-nb-x))
                         (list local (qa-nb-observe))))
                      '(qa-nb-x) '(argument)) '(inner (inner second)))
      (qa (qa-nb-run '((let ((qa-nb-x 'inner)) (qa-nb-observe)))) '(inner second))
      (qa (qa-nb-run '((let ((qa-nb-alias 'alias)) (qa-nb-observe)))) '(alias second))
      (qa (qa-nb-run '((let ((qa-nb-x 'inner)) (setq qa-nb-x 'updated) qa-nb-x))) 'updated)
      (qa qa-nb-x 'outer)

      ;; Parallel initializers run before any dynamic binding is installed.
      (qa (qa-nb-run '((let ((qa-nb-x (progn (setq qa-nb-x 'changed) 'inner))
                             (local qa-nb-x)) (list qa-nb-x local)))) '(inner changed))
      (qa qa-nb-x 'changed)
      (setq qa-nb-x 'outer)

      (qa (qa-nb-run '((qa-nb-observe)) '(qa-nb-x) '(argument)) '(outer second))
      (qa (qa-nb-run '((list value (qa-nb-observe)))
                      '(value &optional qa-nb-x &rest qa-nb-y) '(1 option 3 4)) '(1 (outer second)))
      (qa (qa-nb-run '((list value (qa-nb-observe)))
                      '(value &optional qa-nb-x &rest qa-nb-y) '(1)) '(1 (outer second)))
      (qa (qa-nb-observe) '(outer second))
      (qa (qa-nb-run '((qa-nb-read-free)) '(qa-nb-free) '(argument) t) 'argument)
      (qa (qa-nb-run '((let ((qa-nb-free 'inside)) (qa-nb-read-free))) nil nil t) 'inside)
      (qa (qa-nb-run '((let ((qa-nb-free 'lexical)) (qa-nb-read-free)))) 'outside)
      (qa qa-nb-free 'outside)
      (qa (qa-nb-run '((let ((t 1)) 2))) 2)

      (qa (let ((qa-nb-function (qa-nb-compile '((let ((qa-nb-x 'inner)) qa-nb-x))))
                qa-nb-events
                (watcher (lambda (_symbol value operation _where)
                           (push (list operation value) qa-nb-events))))
            (unwind-protect
                (progn
                  (add-variable-watcher 'qa-nb-x watcher)
                  (list (funcall qa-nb-function) (nreverse qa-nb-events) qa-nb-x))
              (remove-variable-watcher 'qa-nb-x watcher)))
          '(inner nil outer))
      (qa (let ((qa-nb-function
                 (let ((byte-optimize nil))
                   (qa-nb-compile '((let ((qa-nb-x 'inner)) qa-nb-x)))))
                qa-nb-events
                (watcher (lambda (_symbol value operation _where)
                           (push (list operation value) qa-nb-events))))
            (unwind-protect
                (progn
                  (add-variable-watcher 'qa-nb-x watcher)
                  (list (funcall qa-nb-function) (nreverse qa-nb-events) qa-nb-x))
              (remove-variable-watcher 'qa-nb-x watcher)))
          '(inner ((let inner) (unlet outer)) outer))

      (qa (let ((qa-nb-function (qa-nb-compile '((let* ((qa-nb-y 'bound)
                                                       (qa-nb-x 'never))
                                                  (setq qa-nb-side t)))))
                (qa-nb-side nil)
                (watcher (lambda (_symbol _value operation _where)
                           (when (eq operation 'let) (error "binding failure")))))
            (unwind-protect
                (progn
                  (add-variable-watcher 'qa-nb-x watcher)
                  (list (condition-case err (funcall qa-nb-function) (error err))
                        qa-nb-side (qa-nb-observe)))
              (remove-variable-watcher 'qa-nb-x watcher)))
          '((error "binding failure") nil (outer second)))

      (qa (let ((qa-nb-function (qa-nb-compile '((let* ((qa-nb-y 'bound)
                                                       (local (error "initializer")))
                                                  (setq qa-nb-side t)))))
                (qa-nb-side nil) qa-nb-events
                (signal-hook-function
                 (lambda (symbol _data) (when (eq symbol 'error) (push qa-nb-y qa-nb-events)))))
            (list (condition-case err (funcall qa-nb-function) (error err))
                  qa-nb-events qa-nb-side (qa-nb-observe)))
          '((error "initializer") (bound) nil (outer second)))

      (qa (let ((qa-nb-function (qa-nb-compile '((let ((qa-nb-x 'inner)) (qa-nb-observe)))))
                (watcher (lambda (_symbol _value operation _where)
                           (when (eq operation 'unlet) (error "unbinding failure")))))
            (unwind-protect
                (progn
                  (add-variable-watcher 'qa-nb-x watcher)
                  (list (condition-case err (funcall qa-nb-function) (error err)) qa-nb-x))
              (remove-variable-watcher 'qa-nb-x watcher)
              (setq qa-nb-x 'outer))) '((error "unbinding failure") inner))

      ;; These must execute as native code; the old fallback rebuilt a closure.
      (qa (let ((qa-nb-function (qa-nb-compile '((let ((qa-nb-x 'inner)) (qa-nb-observe)))))
                (saved (symbol-function 'make-interpreted-closure)))
            (unwind-protect
                (progn
                  (fset 'make-interpreted-closure (lambda (&rest _) (error "interpreted fallback")))
                  (list (native-comp-function-p qa-nb-function) (funcall qa-nb-function)))
              (fset 'make-interpreted-closure saved))) '(t (inner second)))

      (qa (let ((saved (symbol-function 'byte-optimize-letX)))
            (unwind-protect
                (catch 'optimizer-stop
                  (fset 'byte-optimize-letX (lambda (&rest _) (throw 'optimizer-stop 'stopped)))
                  (qa-nb-compile '((let ((qa-nb-x 'inner)) (qa-nb-observe)))))
              (fset 'byte-optimize-letX saved))) 'stopped)

      (setq qa-nb-function
            (qa-nb-compile '((let ((inhibit-quit nil)) (setq quit-flag t) 'safe))))
      (qa (qa-nb-deferred qa-nb-function) '(safe t))
      (setq qa-nb-function (qa-nb-compile '((setq quit-flag t inhibit-quit nil) qa-nb-x)))
      (qa (qa-nb-deferred qa-nb-function) '(outer t))

      ;; Disabled trampolines leave existing native reads on the C primitive.
      (let ((saved (symbol-function 'symbol-value))
            (native-comp-enable-subr-trampolines nil))
        (unwind-protect
            (progn
              (fset 'symbol-value (lambda (_) 'replacement))
              (qa (qa-nb-deferred qa-nb-function) '(outer t)))
          (fset 'symbol-value saved)))
      (let ((saved (symbol-function 'symbol-value))
            (qa-nb-reader (qa-nb-compile '(qa-nb-x))))
        (unwind-protect
            (progn
              (fset 'symbol-value (lambda (_) 'replacement))
              (qa (funcall qa-nb-reader) 'replacement)
              (fset 'symbol-value saved)
              ;; Once installed, GNU retains its trampoline after restoration.
              (qa (qa-nb-deferred qa-nb-function) '(caught nil)))
          (fset 'symbol-value saved))))
  (delete-directory qa-nb-directory t))
(princ (qa--done)) (terpri)
