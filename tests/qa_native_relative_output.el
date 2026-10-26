;;; Native relative output follows Lisp default-directory. -*- lexical-binding: nil; -*-
;; #663: an owned cwd sentinel ensures a regression cannot overwrite user files.
(let* ((process-directory default-directory)
       (sentinel (make-temp-file (expand-file-name "qa-native-relative-" process-directory) nil ".eln"))
       (directory (make-temp-file "qa-native-relative-default-" t))
       (output (file-name-nondirectory sentinel)))
  (unwind-protect
      (progn
        (with-temp-file sentinel (insert "sentinel"))
        (let* ((default-directory (file-name-as-directory directory))
               (compiled (native-compile (lambda () 42) output)))
          (qa (native-comp-function-p compiled) t)
          (qa (funcall compiled) 42)
          (qa (file-exists-p (expand-file-name output)) t)
          (qa (with-temp-buffer
                (insert-file-contents-literally sentinel)
                (equal (buffer-string) "sentinel")) t)))
    (delete-file sentinel)
    (delete-directory directory t)))
(princ (qa--done))
