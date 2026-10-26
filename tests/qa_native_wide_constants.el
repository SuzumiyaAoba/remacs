;;; Preserve native literals across the full Emacs character domain. -*- lexical-binding: nil; -*-
;; Regression for #624. Requires actual native compilation.
(let ((directory (make-temp-file "qa-native-wide-" t))
      (serial 0))
  (unwind-protect
      (progn
        (unless (native-comp-available-p)
          (error "Actual native compilation is required"))
        (dolist (codes '((983168) (55296) (1703936) (4194176)
                         (65 983168 55296 1703936 4194176 0 66)))
          (let* ((text (string-as-multibyte (apply #'string codes)))
                 (function (eval (list 'lambda nil (list 'quote text)) t))
                 (output (expand-file-name (format "constant-%d.eln" serial) directory))
                 (compiled (native-compile function output)))
            (setq serial (1+ serial))
            (qa (native-comp-function-p compiled) t)
            ;; GNU normalizes a literal containing only byte8 to unibyte.
            (qa (string-to-list (funcall compiled))
                (if (equal codes '(4194176)) '(128) codes))
            (qa (multibyte-string-p (funcall compiled))
                (unless (equal codes '(4194176)) (multibyte-string-p text)))))
        (let* ((text (unibyte-string 0 128 255 65))
               (function (eval (list 'lambda nil (list 'quote text)) t))
               (compiled (native-compile function (expand-file-name "unibyte.eln" directory))))
          (qa (native-comp-function-p compiled) t)
          (qa (string-to-list (funcall compiled)) '(0 128 255 65))
          (qa (multibyte-string-p (funcall compiled)) nil)))
    (delete-directory directory t)))
(princ (qa--done))
