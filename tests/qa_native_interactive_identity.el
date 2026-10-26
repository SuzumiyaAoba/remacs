;;; Native interactive metadata follows function identity. -*- lexical-binding: nil; -*-
;; #651: multiple anonymous functions share a name, never interactive state.
(let ((directory (make-temp-file "qa-native-interactive-identity-" t)))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (let ((first (native-compile (lambda (value) (interactive "sFirst: ") value)
                                    (expand-file-name "first.eln" directory)))
              (second (native-compile (lambda (value) (interactive "sSecond: ") value)
                                     (expand-file-name "second.eln" directory)))
              (plain (native-compile (lambda (value) value)
                                    (expand-file-name "plain.eln" directory))))
          (qa (interactive-form first) '(interactive "sFirst: "))
          (qa (interactive-form second) '(interactive "sSecond: "))
          (qa (interactive-form plain) nil)
          (qa (commandp first) t)
          (qa (commandp second) t)
          (qa (commandp plain) nil)
          (qa (funcall first 1) 1)
          (qa (funcall second 2) 2)
          (qa (funcall plain 3) 3)))
    (delete-directory directory t)))
(princ (qa--done))
