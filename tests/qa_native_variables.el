;;; -*- lexical-binding: t; -*-
;; Issue #183: native free-variable reads fetch the current value cell.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defvar qa183-directory (make-temp-file "remacs-qa-native-variables-" t))
(defvar qa183-serial 0)
(defvar qa183-global 1)
(defvar qa183-reader nil)
(defvar qa183-literals nil)
(defvar qa183-side nil)
(defvar qa183-trace nil)
(defvar qa183-local 'default)
(defvar qa183-alias)
(defvaralias 'qa183-alias 'qa183-global)
(setq qa183-free 'global)
(defun qa183-named () qa183-global)
(defun qa183-compile (body &optional args)
  (setq qa183-serial (1+ qa183-serial))
  (native-compile
   (eval (cons 'lambda (cons args body)) t)
   (expand-file-name (format "case%d.eln" qa183-serial) qa183-directory)))
(defun qa183-mark (value)
  (setq qa183-trace (append qa183-trace (list value)))
  value)
(defun qa183-run (body)
  (setq qa183-side nil qa183-trace nil)
  (let* ((function (qa183-compile body))
         (result (condition-case err (list 'ok (funcall function))
                   (error err))))
    (list result qa183-side qa183-trace)))

(unwind-protect
    (let ((user-emacs-directory qa183-directory)
          (native-comp-eln-load-path (list qa183-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_variables requires native compilation"))
      ;; The original reproduction, then reuse the same native function
      ;; after updates: a variable's value is not a compile-time constant.
      (setq qa183-reader (qa183-compile '(qa183-global)))
      (qa (native-comp-function-p qa183-reader) t)
      (qa (funcall qa183-reader) 1)
      (setq qa183-global 7)
      (qa (funcall qa183-reader) 7)
      (setq qa183-global nil)
      (qa (funcall qa183-reader) nil)
      (setq qa183-global 'value)
      (qa (funcall qa183-reader) 'value)
      (setq qa183-global (list "value" (vector 3)))
      (qa (eq (funcall qa183-reader) qa183-global) t)
      (qa (let ((qa183-global 'dynamic)) (funcall qa183-reader)) 'dynamic)
      (qa (eq (funcall qa183-reader) qa183-global) t)

      ;; A free variable must not capture the calling interpreter's
      ;; lexical environment, even when the variable was not defvar'd.
      (setq qa183-reader (qa183-compile '(qa183-free)))
      (qa-lex (let ((qa183-free 'caller)) (funcall qa183-reader)) 'global)
      (setq qa183-free 'updated)
      (qa (funcall qa183-reader) 'updated)

      ;; Aliases and buffer-local values are resolved at call time.
      (setq qa183-global 11)
      (setq qa183-reader (qa183-compile '(qa183-alias)))
      (qa (funcall qa183-reader) 11)
      (qa (let ((qa183-global 12)) (funcall qa183-reader)) 12)
      (setq qa183-reader (qa183-compile '(qa183-local)))
      (qa (with-temp-buffer
            (set (make-local-variable 'qa183-local) 'buffer)
            (funcall qa183-reader))
          'buffer)
      (qa (funcall qa183-reader) 'default)

      ;; Lexical locals and literal/function symbols keep their meaning.
      (qa (funcall (qa183-compile '((list argument
                                         (let ((local 4)) local)
                                         nil t :keyword 'quoted #'car))
                                 '(argument))
                   3)
          '(3 4 nil t :keyword quoted car))
      (qa (funcall (qa183-compile '((+ qa183-global 2)))) 13)
      (qa (funcall (qa183-compile '((setq qa183-global 21) qa183-global))) 21)
      (qa qa183-global 21)

      ;; GNU native code honors replacement of symbol-value through its
      ;; subr trampoline, while self-evaluating and quoted symbols stay data.
      (setq qa183-reader (qa183-compile '(qa183-global)))
      (setq qa183-literals (qa183-compile '((list nil t :keyword 'quoted #'car))))
      (let ((original (symbol-function 'symbol-value)))
        (unwind-protect
            (progn
              (fset 'symbol-value (lambda (_) 'replacement))
              (qa (funcall qa183-reader) 'replacement)
              (qa (funcall qa183-literals) '(nil t :keyword quoted car)))
          (fset 'symbol-value original)))

      ;; Unbound variables signal before any later form or argument runs.
      (makunbound 'qa183-missing)
      (qa (qa183-run '(qa183-missing (setq qa183-side t)))
          '((void-variable qa183-missing) nil nil))
      (qa (qa183-run '((list (qa183-mark 'before) qa183-missing
                             (qa183-mark 'after))))
          '((void-variable qa183-missing) nil (before)))
      (qa (qa183-run '((if qa183-missing (setq qa183-side 'then)
                         (setq qa183-side 'else))))
          '((void-variable qa183-missing) nil nil))
      (qa (qa183-run '((setq qa183-side qa183-missing)))
          '((void-variable qa183-missing) nil nil))
      ;; A previously compiled reader notices makunbound and later recovery.
      (makunbound 'qa183-global)
      (qa-err-data (funcall qa183-reader) 'void-variable '(qa183-global))
      (setq qa183-global 31)
      (qa (funcall qa183-reader) 31)
      (qa (qa183-run '((and nil qa183-missing))) '((ok nil) nil nil))
      (qa (qa183-run '((or t qa183-missing))) '((ok t) nil nil))
      ;; Symbol and file entry points use the same value-reference codegen.
      (qa (funcall (native-compile
                    'qa183-named (expand-file-name "named.eln" qa183-directory)))
          31)
      (let ((source (expand-file-name "read.el" qa183-directory))
            (output (expand-file-name "read.eln" qa183-directory)))
        (with-temp-file source
          (insert ";;; -*- lexical-binding: t; -*-\n"
                  "(defvar qa183-global)\n"
                  "(defun qa183-file-read () qa183-global)\n"))
        (native-compile source output)
        (load output nil t)
        (qa (native-comp-function-p (symbol-function 'qa183-file-read)) t)
        (qa (qa183-file-read) 31)
        (setq qa183-global 41)
        (qa (qa183-file-read) 41)))
  (delete-directory qa183-directory t))
(princ (qa--done)) (terpri)
