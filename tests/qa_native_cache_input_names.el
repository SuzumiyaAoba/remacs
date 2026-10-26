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
          (qa (comp-el-to-eln-rel-filename "relative.el") (comp-el-to-eln-rel-filename file))))
    (delete-directory directory t)))
(princ (qa--done))
