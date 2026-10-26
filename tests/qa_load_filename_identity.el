;;; Load lookup must never substitute a replacement filename. -*- lexical-binding: nil; -*-
;; #767: U+FFFD and an extended character name distinct files.
(let* ((directory (make-temp-file "qa-load-name-" t))
       (replacement (expand-file-name (concat (string 65533) ".el") directory))
       (original (expand-file-name (concat (string 1703936) ".el") directory))
       (require-a (expand-file-name (concat "require-a-" (string 1703936) ".el") directory))
       (require-b (expand-file-name (concat "require-b-" (string 1703936) ".el") directory))
       (qa767-marker nil) (qa767-require-marker nil) (qa767-loaded-code nil)
       (qa767-load-names nil) (qa767-after-load nil))
  (unwind-protect
      (progn
        (with-temp-file replacement
          (insert ";;; -*- lexical-binding: nil; -*-\n(setq qa767-marker t)\n"))
        (dolist (entry '(("require-a-" qa767-feature-a) ("require-b-" qa767-feature-b)))
          (with-temp-file (expand-file-name (concat (car entry) (string 65533) ".el") directory)
            (insert ";;; -*- lexical-binding: nil; -*-\n(setq qa767-require-marker t)\n"
                    (format "(provide '%s)\n" (cadr entry)))))
        (qa (locate-library original t) nil)
        (qa-err (load original nil t t) 'file-missing)
        (qa qa767-marker nil)
        (qa (load original t t t) nil)
        (qa qa767-marker nil)
        (qa-err (load-file original) 'file-missing)
        (qa qa767-marker nil)
        (qa-err (require 'qa767-feature-a require-a) 'file-missing)
        (qa qa767-require-marker nil)
        (qa (featurep 'qa767-feature-a) nil)
        (setq qa767-require-marker nil)
        (qa (require 'qa767-feature-b require-b t) nil)
        (qa qa767-require-marker nil)
        (qa (featurep 'qa767-feature-b) nil)
        ;; A literal replacement filename remains a valid, explicitly chosen file.
        (qa (load replacement nil t t) t)
        (qa qa767-marker t)
        (dolist (code '(983168 983296))
          (let ((file (expand-file-name (concat (string code) ".el") directory)))
            (with-temp-file file
              (insert ";;; -*- lexical-binding: nil; -*-\n"
                      (format "(setq qa767-loaded-code %d)\n" code)
                      "(setq qa767-load-names (list load-file-name load-true-file-name (car current-load-list)))\n"
                      "(defun qa767-loaded-function () t)\n"))
            (let ((after-load-functions
                   (cons (lambda (name) (setq qa767-after-load name)) after-load-functions)))
              (qa (load file nil t t) t))
            (qa qa767-loaded-code code)
            (qa qa767-load-names (list file file file))
            (qa qa767-after-load file)
            (qa (symbol-file 'qa767-loaded-function 'defun) file)
            (qa (car (assoc file load-history)) file)
            (qa (locate-library file t) file)
            (qa (locate-library (file-name-nondirectory file) t (list directory)) file))))
    (dolist (feature '(qa767-feature-a qa767-feature-b))
      (when (featurep feature) (unload-feature feature t)))
    (delete-directory directory t)))
(princ (qa--done))
