;;; qa_elint_compile_scope.el --- Elint compile-time scope regression -*- lexical-binding:t -*-

(require 'elint)

(qa
 (with-temp-buffer
   (insert "(eval-when-compile\n"
           "  (defun qa-elint-compile-only-helper () nil)\n"
           "  (defvar qa-elint-compile-only-variable nil)\n"
           "  (defmacro qa-elint-compile-only-macro () nil))\n")
   (elint-update-env)
   (list (elint-env-find-func elint-buffer-env
                              'qa-elint-compile-only-helper)
         (elint-env-find-var elint-buffer-env
                             'qa-elint-compile-only-variable)
         (elint-env-find-func elint-buffer-env
                              'qa-elint-compile-only-macro)
         (and (elint-env-macrop elint-buffer-env
                                'qa-elint-compile-only-macro)
              t)))
 '(nil nil nil t))

(princ (qa--done))
