;;; -*- lexical-binding: t; -*-
;; Issue #190: native string constants retain bytes and multibyte metadata.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defmacro qa190-check (form expected)
  `(let ((actual (condition-case err ,form
                   (error (list 'qa--err (car err) (cdr err)))))
         (wanted ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal actual wanted)
       (push (list ',form wanted actual) qa--fails))))
(defvar qa190-directory (make-temp-file "remacs-qa-native-strings-" t))
(defvar qa190-serial 0)
(defvar qa190-loaded nil)
(defun qa190-compile (fn)
  (setq qa190-serial (1+ qa190-serial))
  (native-compile fn (expand-file-name (format "lambda%d.eln" qa190-serial)
                                      qa190-directory)))
(defun qa190-shape (string)
  (list (multibyte-string-p string) (string-to-list string)
        (length string) (string-bytes string)))

(unwind-protect
    (let ((user-emacs-directory qa190-directory)
          (native-comp-eln-load-path (list qa190-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_strings requires native compilation"))
      ;; The reported octal-byte case, including the uncompiled control.
      (qa190-check (qa190-shape "\200\377") '(nil (128 255) 2 2))
      (let ((fn (qa190-compile (lambda () "\200\377"))))
        (qa190-check (native-comp-function-p fn) t)
        (qa190-check (qa190-shape (funcall fn)) '(nil (128 255) 2 2)))

      ;; Direct literals include every byte, escaped controls, Unicode and
      ;; surrogate characters.
      ;; Constructing the lambda at runtime avoids testing the source reader
      ;; instead of the native emitter for the surrogate and byte-kind cases.
      (dolist (literal (list "" "ascii" "\0\n\t\r\"\\7" "\177\200\377"
                            (apply #'unibyte-string (number-sequence 0 255))
                            (unibyte-string 0 127 128 255 0)
                            "café-日本-😀" "é\200\377"
                            (string 55296) (string 56320)
                            (string 0 34 92 10 233 55296 56320)))
        (let* ((wanted (qa190-shape literal))
               (fn (qa190-compile (eval (list 'lambda nil literal) t)))
               (first (funcall fn)))
          (qa190-check (native-comp-function-p fn) t)
          (qa190-check (qa190-shape first) wanted)
          (qa190-check (equal first literal) t)
          (qa190-check (eq first (funcall fn)) t)
          (garbage-collect)
          (qa190-check (eq first (funcall fn)) t)
          (qa190-check (qa190-shape (funcall fn)) wanted)))

      ;; Cached byte strings remain mutable and retain their byte semantics.
      (let* ((fn (qa190-compile (lambda () "\200\377")))
             (first (funcall fn)))
        (aset first 0 254)
        (qa190-check (eq first (funcall fn)) t)
        (qa190-check (qa190-shape (funcall fn)) '(nil (254 255) 2 2)))

      ;; The read-constant and fallback paths must continue preserving bytes.
      (dolist (form '((lambda () '"\200\377")
                      (lambda () (unwind-protect "\200\377" nil))
                      (lambda () (condition-case nil "\200\377" (error nil)))))
        (let ((fn (qa190-compile (eval form t))))
          (qa190-check (native-comp-function-p fn) t)
          (qa190-check (qa190-shape (funcall fn)) '(nil (128 255) 2 2))
          (qa190-check (eq (funcall fn) (funcall fn)) t)))

      ;; File compilation and repeated loads cover both source binding modes.
      (dolist (dynamic '(nil t))
        (let* ((source (expand-file-name (if dynamic "dynamic.el" "lexical.el")
                                        qa190-directory))
               (output (concat source "n")))
          (with-temp-file source
            (insert (if dynamic ";;; -*- lexical-binding: nil; -*-\n"
                      ";;; -*- lexical-binding: t; -*-\n")
                    "(defun qa190-file () \"\\200\\377\")\n"
                    "(defun qa190-file-fallback () (unwind-protect \"\\200\\377\" nil))\n"
                    "(setq qa190-loaded (string-to-list (qa190-file)))\n"))
          (setq qa190-loaded 'before-load)
          (native-compile source output)
          (qa190-check qa190-loaded 'before-load)
          (native-elisp-load output)
          (qa190-check qa190-loaded '(128 255))
          (qa190-check (native-comp-function-p (symbol-function 'qa190-file)) t)
          (qa190-check (qa190-shape (qa190-file)) '(nil (128 255) 2 2))
          (qa190-check (qa190-shape (qa190-file-fallback)) '(nil (128 255) 2 2))
          (let* ((old (symbol-function 'qa190-file))
                 (first (funcall old)))
            (native-elisp-load output)
            (qa190-check (eq first (funcall old)) t)
            (qa190-check (qa190-shape (funcall old)) '(nil (128 255) 2 2))
            (qa190-check (qa190-shape (qa190-file)) '(nil (128 255) 2 2))
            (qa190-check (eq (qa190-file) (qa190-file)) t)))))
  (delete-directory qa190-directory t))

(princ (concat (qa--done) "\n"))
