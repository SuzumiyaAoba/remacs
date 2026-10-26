;;; Preserve native interactive descriptors. -*- lexical-binding: nil; -*-
;; #650: encoded lengths count bytes; metadata reading must retain Emacs chars.
(let ((directory (make-temp-file "qa-native-wide-interactive-" t))
      (serial 0)
      (names nil))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (dolist (code '(233 983168 55296 1703936))
          (let* ((prompt (concat "sPrompt-" (string code) ": "))
                 (name (intern (format "qa-native-wide-interactive-%d" serial)))
                 (function (eval (list 'lambda '(value) (list 'interactive prompt) 'value) t)))
            (push name names)
            (fset name function)
            (let ((compiled (native-compile name
                             (expand-file-name (format "interactive-%d.eln" serial) directory))))
              (setq serial (1+ serial))
              (qa (native-comp-function-p compiled) t)
              (qa (funcall compiled 42) 42)
              (qa (commandp compiled) t)
              (qa (string-to-list (cadr (interactive-form compiled))) (string-to-list prompt))))))
    (dolist (name names) (fmakunbound name))
    (delete-directory directory t)))
(princ (qa--done))
