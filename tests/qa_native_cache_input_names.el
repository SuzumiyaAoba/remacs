;;; Native cache keys must refer to the actual input filename. -*- lexical-binding: nil; -*-
;; #667: a replacement-named file must never stand in for the requested source.
(require 'comp)
(let* ((directory (make-temp-file "qa-native-cache-input-" t))
       (replacement (expand-file-name (concat (string 65533) ".el") directory))
       (original (expand-file-name (concat (string 1703936) ".el") directory)))
  (unwind-protect
      (progn
        (with-temp-file replacement (insert "(defun qa-cache-source () 42)\n"))
        (qa-err (comp-el-to-eln-rel-filename original) 'file-missing)
        (qa-err (comp-el-to-eln-filename original) 'file-missing)
        (qa (file-exists-p replacement) t)
        (dolist (code '(983168 983296 57344))
          (let* ((base (concat "source-" (string code)))
                 (file (expand-file-name (concat base ".el") directory)))
            (with-temp-file file (insert "(defun qa-cache-source () 42)\n"))
            (qa (string-prefix-p base (file-name-nondirectory (comp-el-to-eln-rel-filename file))) t)))
        (let* ((file (expand-file-name "relative.el" directory))
               (default-directory (file-name-as-directory directory)))
          (with-temp-file file (insert "(defun qa-cache-relative () 42)\n"))
          (qa (comp-el-to-eln-rel-filename "relative.el") (comp-el-to-eln-rel-filename file)))
        (let* ((target (make-temp-file "qa-native-cache-target-" t))
               (file (expand-file-name "output.el" directory)))
          (unwind-protect
              (progn
                (with-temp-file file (insert "(defun qa-cache-output () 42)\n"))
                (let* ((native-comp-eln-load-path (list directory target))
                       (relative (comp-el-to-eln-rel-filename file)))
                  (qa (comp-el-to-eln-filename file)
                      (expand-file-name
                       (concat comp-native-version-dir "/" relative) directory))
                  (qa (comp-el-to-eln-filename file target)
                      (expand-file-name
                       (concat comp-native-version-dir "/" relative) target)))
                (qa-err (let ((native-comp-eln-load-path nil))
                          (comp-el-to-eln-filename file))
                        'error))
            (delete-directory target t))))
        (when (native-comp-available-p)
          (let* ((cache (make-temp-file "qa-native-cache-compile-" t))
                 (source (expand-file-name "compiled.el" cache))
                 output)
            (unwind-protect
                (progn
                  (with-temp-file source
                    (insert ";;; -*- lexical-binding: t; -*-\n"
                            "(defun qa-cache-native-output () 42)\n"))
                  (let ((native-comp-eln-load-path (list cache)))
                    (setq output (native-compile source))
                    (qa (file-name-directory output)
                        (expand-file-name
                         (concat comp-native-version-dir "/") cache))
                    (qa (file-exists-p output) t)
                    (native-elisp-load output)
                    (qa (qa-cache-native-output) 42)))
              (when (and output (file-exists-p output))
                (delete-file output))
              (delete-directory cache t))))
    (delete-directory directory t)))
(princ (qa--done))
