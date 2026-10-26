;;; Native file compilation shares source decoding. -*- lexical-binding: nil; -*-
;; #669: read logical Emacs characters, not Rust UTF-8 strings.
(let ((directory (make-temp-file "qa-native-source-coding-" t))
      (serial 0))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (dolist (case '(("utf-8-emacs" (246 160 128 128) (1703936))
                        ("utf-8-emacs" (237 160 128) (237 160 128))
                        ("utf-8-emacs" (243 176 130 128) (983168))
                        ("utf-8-emacs" (65 243 176 130 128 237 160 128 246 160 128 128 66)
                                      (65 983168 4194285 4194208 4194176 1703936 66))
                        ("iso-latin-1-unix" (233) (233))))
          (let ((source (expand-file-name (format "source-%d.el" serial) directory))
                (output (expand-file-name (format "source-%d.eln" serial) directory)))
            (setq serial (1+ serial))
            (with-temp-buffer
              (set-buffer-multibyte nil)
              (insert (format ";;; -*- lexical-binding: t; coding: %s; -*-\n(defun qa-native-coded-source () \"" (car case))
                      (apply #'unibyte-string (nth 1 case)) "\")\n")
              (let ((coding-system-for-write 'no-conversion))
                (write-region (point-min) (point-max) source nil 'silent)))
            (native-elisp-load (native-compile source output))
            (qa (native-comp-function-p (symbol-function 'qa-native-coded-source)) t)
            (qa (string-to-list (qa-native-coded-source)) (nth 2 case))))
        (let ((source (expand-file-name "utf16.el" directory))
              (output (expand-file-name "utf16.eln" directory))
              (coding-system-for-write 'utf-16le-with-signature))
          (with-temp-file source
            (insert ";;; -*- lexical-binding: t; coding: utf-16le-with-signature; -*-\n"
                    "(defun qa-native-coded-source () \"" (string 983168) "\")\n"))
          (native-elisp-load (native-compile source output))
          (qa (native-comp-function-p (symbol-function 'qa-native-coded-source)) t)
          (qa (string-to-list (qa-native-coded-source)) '(983168))))
    (fmakunbound 'qa-native-coded-source)
    (delete-directory directory t)))
(princ (qa--done))
