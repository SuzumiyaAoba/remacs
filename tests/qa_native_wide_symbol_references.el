;;; Native symbol references retain names and their representation. -*- lexical-binding: nil; -*-
;; #656: call targets, free variables, setq, function names and dynamic binds.
(let ((directory (make-temp-file "qa-native-symbol-refs-" t))
      (serial 0) (symbols nil))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (dolist (code '(983168 55296 1703936))
          (let* ((suffix (string code))
                 (callee (intern (concat "qa-native-callee-" suffix)))
                 (variable (intern (concat "qa-native-variable-" suffix))))
            (push callee symbols)
            (push variable symbols)
            (fset callee (lambda () 42))
            (eval (list 'defvar variable))
            (set variable 7)
            (let* ((reader (native-compile
                            (eval (list 'lambda nil
                                        (list 'list (list callee) variable (list 'function callee))) t)
                            (expand-file-name (format "read-%d.eln" serial) directory)))
                   (writer (native-compile
                            (eval (list 'lambda '(value) (list 'setq variable 'value)) t)
                            (expand-file-name (format "write-%d.eln" serial) directory)))
                   (binder (native-compile
                            (eval (list 'lambda '(value)
                                        (list 'let (list (list variable 'value)) variable)) t)
                            (expand-file-name (format "bind-%d.eln" serial) directory))))
              (setq serial (1+ serial))
              (qa (native-comp-function-p reader) t)
              (qa (native-comp-function-p writer) t)
              (qa (native-comp-function-p binder) t)
              (if (= code 55296)
                  ;; GNU native relocations expose surrogate C spellings as unibyte names.
                  (progn
                    (qa-err (funcall reader) 'void-function)
                    (qa (condition-case failure (funcall reader)
                          (void-function (string-to-list (symbol-name (cadr failure)))))
                        (append (string-to-list "qa-native-callee-") '(193 173 192 160 192 128)))
                    (qa (condition-case failure (funcall reader)
                          (void-function (multibyte-string-p (symbol-name (cadr failure))))) nil))
                (qa (car (funcall reader)) 42)
                (qa (nth 1 (funcall reader)) 7)
                (qa (eq (nth 2 (funcall reader)) callee) t))
              (qa (funcall writer 11) 11)
              (qa (symbol-value variable) (if (= code 55296) 7 11))
              (qa (funcall binder 99) 99)
              (qa (symbol-value variable) (if (= code 55296) 7 11))))))
    (dolist (symbol symbols) (fmakunbound symbol) (makunbound symbol))
    (delete-directory directory t)))
(princ (qa--done))
