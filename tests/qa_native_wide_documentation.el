;;; Native function documentation preserves extended characters. -*- lexical-binding: nil; -*-
;; #647: native descriptor text is logical Emacs text, not Rust UTF-8.
(let ((directory (make-temp-file "qa-native-wide-doc-" t))
      (serial 0))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (dolist (code '(983168 55296 1703936))
          (let* ((doc (concat "doc-" (string code)))
                 (function (eval (list 'lambda nil doc nil) t))
                 (compiled (native-compile function
                            (expand-file-name (format "doc-%d.eln" serial) directory))))
            (setq serial (1+ serial))
            (qa (native-comp-function-p compiled) t)
            (qa (string-to-list (documentation compiled t)) (string-to-list doc))))
        (let ((compiled (native-compile (lambda () nil)
                         (expand-file-name "no-doc.eln" directory))))
          (qa (documentation compiled t) nil)))
    (delete-directory directory t)))
(princ (qa--done))
