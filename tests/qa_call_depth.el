;;; eval, funcall and Bcall share GNU's nesting counter. -*- lexical-binding: t; -*-
(require 'bytecomp)
(defvar qa-cd-count 0)
(defvar qa-cd-callee 'qa-cd-count-rec)
(defun qa-cd-run (form compiled)
  (let ((definition (list 'lambda nil form)) (byte-optimize t))
    (fset 'qa-cd-rec (if compiled (byte-compile definition) (eval definition t))))
  (let ((max-lisp-eval-depth 110) (text-quoting-style 'curve))
    (condition-case err (qa-cd-rec) (error err))))

(qa (qa-cd-run '(qa-cd-rec) nil) '(excessive-lisp-nesting 111))
(qa (qa-cd-run '(funcall #'qa-cd-rec) nil) '(excessive-lisp-nesting 111))
(qa (qa-cd-run '(apply #'qa-cd-rec nil) nil) '(excessive-lisp-nesting 111))
(qa (qa-cd-run '(qa-cd-rec) t) '(error "Lisp nesting exceeds ‘max-lisp-eval-depth’"))
(qa (qa-cd-run '(funcall #'qa-cd-rec) t) '(error "Lisp nesting exceeds ‘max-lisp-eval-depth’"))
(qa (qa-cd-run '(apply #'qa-cd-rec nil) t) '(error "Lisp nesting exceeds ‘max-lisp-eval-depth’"))

;; A dedicated opcode neither consumes another call depth nor consults
;; a redefined primitive's function cell.
(qa (let* ((byte-optimize t)
           (function (byte-compile '(lambda (value) (length value))))
           (saved (symbol-function 'length)))
      (unwind-protect
          (progn
            (fset 'length (lambda (_) 'overridden))
            (funcall function '(a b)))
        (fset 'length saved))) 2)

(qa (let* ((byte-optimize t)
           (function (byte-compile '(lambda (value) (string= value "same"))))
           (saved (symbol-function 'string-equal)))
      (unwind-protect
          (progn (fset 'string-equal (lambda (&rest _) nil)) (funcall function "same"))
        (fset 'string-equal saved))) t)
(qa (let* ((byte-optimize t)
           (function (byte-compile '(lambda (value) (string< value "z"))))
           (saved (symbol-function 'string-lessp)))
      (unwind-protect
          (progn (fset 'string-lessp (lambda (&rest _) nil)) (funcall function "a"))
        (fset 'string-lessp saved))) t)
(qa (let* ((byte-optimize t)
           (function (byte-compile '(lambda (value) (length value))))
           (saved (symbol-function 'length)))
      (unwind-protect
          (progn (fmakunbound 'length) (funcall function '(a b)))
        (fset 'length saved))) 2)

;; Body-entry counts pin the additional funcall level as well as the limit.
(defun qa-cd-count-rec (indirect)
  (setq qa-cd-count (1+ qa-cd-count))
  (if indirect (funcall qa-cd-callee indirect) (qa-cd-count-rec indirect)))
(defun qa-cd-count-run ()
  (let ((max-lisp-eval-depth 110) (qa-cd-count 0))
      (condition-case nil (qa-cd-count-rec nil) (error nil))
      (let ((direct qa-cd-count))
        (setq qa-cd-count 0)
        (condition-case nil (qa-cd-count-rec t) (error nil))
        (list direct qa-cd-count))))
(qa (qa-cd-count-run) '(49 32))

(qa (let ((file (make-temp-file "qa-call-depth-autoload-" nil ".el")))
      (unwind-protect
          (progn
            (with-temp-file file
              (insert ";;; -*- lexical-binding: t; -*-\n"
                      "(defun qa-cd-auto ()\n"
                      "  (not (null (memq 'qa-cd-auto (mapcar #'cadr (backtrace-frames))))))\n"))
            (autoload 'qa-cd-auto file)
            (funcall 'qa-cd-auto))
        (fmakunbound 'qa-cd-auto)
        (delete-file file))) t)
(princ (qa--done)) (terpri)
