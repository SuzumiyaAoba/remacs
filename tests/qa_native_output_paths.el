;;; Native compilation must use the requested filename. -*- lexical-binding: nil; -*-
;; #661: never write a U+FFFD replacement pathname on conversion failure.
(let ((directory (make-temp-file "qa-native-output-paths-" t))
      (replacement nil))
  (setq replacement (expand-file-name (concat (string 65533) ".eln") directory))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (dolist (code '(983168 983296 57344))
          (let* ((output (expand-file-name (concat (string code) ".eln") directory))
                 (compiled (native-compile (lambda () 42) output)))
            (qa (native-comp-function-p compiled) t)
            (qa (file-exists-p output) t)
            (qa (file-exists-p replacement) nil)
            (qa (funcall compiled) 42)))
        ;; APFS rejects this non-Unicode spelling. Preserve the actual OS error.
        (when (eq system-type 'darwin)
          (let ((output (expand-file-name (concat (string 1703936) ".eln") directory)))
            (qa-err (native-compile (lambda () 42) output) 'file-error)
            (qa (file-exists-p output) nil)
            (qa (file-exists-p replacement) nil))))
    (delete-directory directory t)))
(princ (qa--done))
