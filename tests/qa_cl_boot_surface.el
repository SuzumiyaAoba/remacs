;;; -*- lexical-binding: t; -*-
;; Issue #87: hidden compiler helpers must not become user-visible
;; macro definitions at -Q. Explicitly load cl-lib before using them.
(defvar qa87-name)
(dolist (qa87-name '(cl-loop cl-tagbody cl-go cl-block cl-flet cl-labels
                    cl-macrolet cl-symbol-macrolet cl-destructuring-bind))
  (qa (fboundp qa87-name) nil))
(qa-err-data (cl-loop return 1) 'void-function '(cl-loop))
(qa-err-data (cl-loop for i from 1 to 5 collect i) 'void-function '(cl-loop))
(qa-err-data (cl-tagbody (cl-go a) a) 'void-function '(cl-tagbody))
(qa-err-data (cl-flet ((f () 1)) (f)) 'void-function '(cl-flet))
(qa (macroexpand '(cl-loop return 1)) '(cl-loop return 1))
(qa (macroexpand-all '(cl-loop return 1)) '(cl-loop return 1))
(qa-err-data (eval '(cl-loop return 1) t) 'void-function '(cl-loop))
(qa-err-data (funcall (lambda () (cl-loop return 1))) 'void-function '(cl-loop))
(qa-err-data (seq-map (lambda (_) (cl-loop return 1)) '(0)) 'void-function '(cl-loop))
(qa-err-data (with-temp-buffer
               (insert ";;; -*- lexical-binding: t; -*-\n(cl-loop return 1)")
               (eval-buffer))
             'void-function '(cl-loop))

(defvar qa87-directory)
(defvar qa87-file)
(let* ((qa87-directory (make-temp-file "qa87-" t))
       (qa87-file (expand-file-name "simple.el" qa87-directory)))
  (unwind-protect
      (progn
        ;; A user override of a shipped basename is still user code.
        (with-temp-file qa87-file
          (insert ";;; -*- lexical-binding: t; -*-\n"
                  "(defun qa87-user (&rest _) (cl-loop return 1))\n"))
        (let ((load-path (cons qa87-directory load-path)))
          (qa (load "simple" nil t) t))
        (qa-err-data (qa87-user) 'void-function '(cl-loop))
        (qa-err-data (seq-map #'qa87-user '(0)) 'void-function '(cl-loop))
        (with-temp-file qa87-file
          (insert ";;; -*- lexical-binding: t; -*-\n(cl-loop return 1)\n"))
        (qa-err-data (load qa87-file nil t) 'void-function '(cl-loop)))
    (delete-directory qa87-directory t)))
(qa (featurep 'cl-lib) nil)
(qa (require 'cl-lib) 'cl-lib)
(qa (cl-loop return 1) 1)
(qa (cl-loop for i from 1 to 5 collect i) '(1 2 3 4 5))
(qa-err-data (cl-tagbody (cl-go a) a) 'void-function '(cl-go))
(qa (cl-tagbody (go a) a) nil)
(qa (cl-flet ((f () 1)) (f)) 1)
(qa (cl-destructuring-bind (a b) '(2 3) (+ a b)) 5)

(princ (qa--done)) (terpri)
