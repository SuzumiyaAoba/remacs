;;; -*- lexical-binding: t; -*-
;; Issue #185: a let* initializer must see the preceding scope.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defvar qa185-directory (make-temp-file "remacs-qa-native-letstar-" t))
(defvar qa185-serial 0)
(defvar qa185-function nil)
(defvar qa185-trace nil)
(defvar qa185-special 10)
(setq qa185-free 20)
(defun qa185-named (x) (let* ((x (1+ x))) x))
(defun qa185-compile (form)
  (setq qa185-serial (1+ qa185-serial))
  (native-compile
   (eval form t)
   (expand-file-name (format "case%d.eln" qa185-serial) qa185-directory)))
(defun qa185-mark (value)
  (setq qa185-trace (append qa185-trace (list value)))
  value)

(unwind-protect
    (let ((user-emacs-directory qa185-directory)
          (native-comp-eln-load-path (list qa185-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_letstar requires native compilation"))
      ;; Exact reported case, repeated calls, and non-numeric values.
      (setq qa185-function
            (qa185-compile '(lambda (x) (let* ((x (1+ x))) x))))
      (qa (native-comp-function-p qa185-function) t)
      (qa (funcall qa185-function 7) 8)
      (qa (list (funcall qa185-function 1) (funcall qa185-function 10)
                (funcall qa185-function 1)) '(2 11 2))
      (qa (funcall (qa185-compile '(lambda (x) (let* ((x x)) x))) 'value)
          'value)
      (qa (funcall (qa185-compile '(lambda (x) (let* ((x x)) x))) nil) nil)
      (qa (let ((object (list 1 2)))
            (eq (funcall (qa185-compile '(lambda (x) (let* ((x x)) x)))
                         object) object)) t)
      (qa (funcall (qa185-compile '(lambda (&optional x)
                                    (let* ((x x)) x))) 4) 4)
      (qa (funcall (qa185-compile '(lambda (&rest x)
                                    (let* ((x x)) x))) 4 5) '(4 5))

      ;; Each binding becomes visible to the following initializer and body.
      (qa (funcall (qa185-compile
                    '(lambda (x) (let* ((x (1+ x)) (x (* x 2)) (y x))
                                   (list x y)))) 7) '(16 16))
      (qa (funcall (qa185-compile
                    '(lambda (x) (let* ((y x) (x (1+ y)) (z x))
                                   (list x y z)))) 7) '(8 7 8))
      (qa (funcall (qa185-compile
                    '(lambda (x) (let* ((x (1+ x)) y) (list x y)))) 7)
          '(8 nil))
      (qa (funcall (qa185-compile
                    '(lambda (x) (let* (x (y x)) (list x y)))) 7) '(nil nil))
      (qa (funcall (qa185-compile
                    '(lambda (x) (let ((x (1+ x)) y) (list x y)))) 7)
          '(8 nil))
      (qa (funcall (qa185-compile '(lambda () (let (y) y)))) nil)
      (qa (funcall (qa185-compile '(lambda () (let* (y) y)))) nil)
      (qa (funcall (qa185-compile '(lambda () (let* (y) 'constant))))
          'constant)
      (qa (funcall (qa185-compile '(lambda (x) (let* () x))) 7) 7)
      (qa (funcall (qa185-compile '(lambda (x) (let* ((x (1+ x)))))) 7) nil)

      ;; Nested initializers see the right old binding, then restore it.
      (qa (funcall (qa185-compile
                    '(lambda (x)
                       (let ((x (1+ x))) (let* ((x (* x 2))) x)))) 7) 16)
      (qa (funcall (qa185-compile
                    '(lambda (x)
                       (let* ((x (let* ((x (1+ x))) (* x 2)))
                              (y (1+ x)))
                         (list x y)))) 7) '(16 17))
      (qa (funcall (qa185-compile
                    '(lambda (x) (list (let* ((x (1+ x))) x) x))) 7)
          '(8 7))
      ;; Parallel let initializers must keep seeing the outer scope.
      (qa (funcall (qa185-compile
                    '(lambda (x) (let ((x (1+ x)) (y x)) (list x y)))) 7)
          '(8 7))
      (qa (funcall (qa185-compile
                    '(lambda (x) (let* ((x (1+ x)))
                                   (let ((x (1+ x)) (y x)) (list x y))))) 7)
          '(9 8))

      ;; An initializer's setq writes the old cell, not its future binding.
      (qa (funcall (qa185-compile
                    '(lambda (x)
                       (list (let* ((x (progn (setq x (1+ x)) (* x 2)))) x)
                             x))) 7) '(16 8))
      (qa (funcall (qa185-compile
                    '(lambda (x)
                       (let* ((x (1+ x))
                              (y (progn (setq x (1+ x)) x)))
                         (list x y)))) 7) '(9 9))
      (setq qa185-trace nil)
      (qa (funcall (qa185-compile
                    '(lambda (x)
                       (let* ((x (qa185-mark (1+ x)))
                              (x (qa185-mark (* x 2))))
                         (qa185-mark x)))) 7) 16)
      (qa qa185-trace '(8 16 16))

      ;; Free/unbound and special-variable initializers keep their semantics.
      (qa (funcall (qa185-compile
                    '(lambda () (let* ((qa185-free (1+ qa185-free)))
                                  qa185-free)))) 21)
      (qa qa185-free 20)
      (makunbound 'qa185-missing)
      (setq qa185-trace nil)
      (qa-err-data
          (funcall (qa185-compile
                    '(lambda () (let* ((qa185-missing qa185-missing)
                                        (later (qa185-mark 'later)))
                                  (qa185-mark 'body)))))
          'void-variable '(qa185-missing))
      (qa qa185-trace nil)
      (qa (funcall (qa185-compile
                    '(lambda () (let* ((qa185-special (1+ qa185-special)))
                                  qa185-special)))) 11)
      (qa qa185-special 10)
      ;; Native closure fallback remains compatible with the direct path.
      (qa (funcall (funcall (qa185-compile
                            '(lambda (x) (let* ((x (1+ x)))
                                           (lambda () x)))) 7)) 8)

      ;; Symbol and lexical file entry points share the same emission.
      (qa (funcall (native-compile
                    'qa185-named (expand-file-name "named.eln" qa185-directory))
                   7) 8)
      (let ((source (expand-file-name "letstar.el" qa185-directory))
            (output (expand-file-name "letstar.eln" qa185-directory)))
        (with-temp-file source
          (insert ";;; -*- lexical-binding: t; -*-\n"
                  "(defun qa185-file (x) "
                  "(let* ((x (1+ x)) (x (* x 2))) x))\n"))
        (native-compile source output)
        (load output nil t)
        (qa (native-comp-function-p (symbol-function 'qa185-file)) t)
        (qa (qa185-file 7) 16)))
  (delete-directory qa185-directory t))
(princ (qa--done)) (terpri)
