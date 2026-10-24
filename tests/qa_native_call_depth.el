;;; Native call boundaries honor GNU's eval-depth guard. -*- lexical-binding: t; -*-
(require 'comp)
(defvar qa-nd-seen nil)
(let* ((directory (make-temp-file "qa-native-depth-" t))
       (user-emacs-directory directory)
       (native-comp-eln-load-path (list directory)))
  (unwind-protect
      (progn
        (fset 'qa-nd-rec
              (native-compile (lambda () (qa-nd-rec))
                              (expand-file-name "recursive.eln" directory)))
        (qa (native-comp-function-p (symbol-function 'qa-nd-rec)) t)
        (qa (let ((max-lisp-eval-depth 110))
              (condition-case err (qa-nd-rec) (error err))) '(excessive-lisp-nesting 111))
        (qa (let ((max-lisp-eval-depth 110) (lisp-eval-depth-reserve 200) qa-nd-seen
                  (signal-hook-function
                   (lambda (symbol data)
                     (setq qa-nd-seen
                           (list symbol data max-lisp-eval-depth lisp-eval-depth-reserve)))))
              (condition-case nil (qa-nd-rec) (excessive-lisp-nesting nil))
              (list qa-nd-seen max-lisp-eval-depth lisp-eval-depth-reserve))
            '((excessive-lisp-nesting (111) 131 179) 110 200)))
    (delete-directory directory t)))
(princ (qa--done)) (terpri)
