;;; -*- lexical-binding: t; -*-
;; Issue #187: malformed file tails must fail before emitting a native unit.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
;; Evaluate in the caller's lexical scope, including state-restoration probes.
(defmacro qa187-check (form expected)
  `(let ((actual (condition-case err ,form
                   (error (list 'qa--err (car err) (cdr err)))))
         (wanted ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal actual wanted)
       (push (list ',form wanted actual) qa--fails))))
(defvar qa187-directory (make-temp-file "remacs-qa-native-reader-" t))
(defvar qa187-serial 0)
(defvar qa187-loaded nil)
(defun qa187-path (extension)
  (expand-file-name (format "case%d.%s" qa187-serial extension) qa187-directory))
(defun qa187-source (body &optional dynamic)
  (setq qa187-serial (1+ qa187-serial))
  (with-temp-file (qa187-path "el")
    (insert (if dynamic ";;; -*- lexical-binding: nil; -*-\n"
              ";;; -*- lexical-binding: t; -*-\n") body))
  (qa187-path "el"))
(defun qa187-result (source output)
  (condition-case err (progn (native-compile source output) 'compiled)
    (error (car err))))
(defmacro qa187-existing-macro () 'original)

(unwind-protect
    (let ((user-emacs-directory qa187-directory)
          (native-comp-eln-load-path (list qa187-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_parse_errors requires native compilation"))
      ;; Exact reported case plus incomplete/invalid syntax after a valid
      ;; prefix. Delimiters in comments and multibyte text do not hide errors.
      (dolist (dynamic '(nil t))
        (dolist (case '(("(\n" end-of-file)
                        ("[1 2" end-of-file)
                        ("\"unfinished" end-of-file)
                        ("\"unfinished\\" end-of-file)
                        ("' ; no quoted object\n" end-of-file)
                        ("#'" end-of-file)
                        ("`" end-of-file)
                        ("(a ." end-of-file)
                        ("(a ; ) is a comment\n" end-of-file)
                        ("; 日本語 ) ]\n(\"é🙂\"" end-of-file)
                        (")" invalid-read-syntax)
                        ("]" invalid-read-syntax)
                        ("(a . b c)" invalid-read-syntax)
                        ("#|" invalid-read-syntax)
                        ("#xg" invalid-read-syntax)
                        ("?\\U00110000" error)
                        ("\"\\U00110000\"" error)))
          (let* ((source (qa187-source
                          (concat "(defun qa187-valid-prefix () 7)\n"
                                  "(setq qa187-loaded t)\n"
                                  "(provide 'qa187-partial-feature)\n"
                                  (car case)) dynamic))
                 (output (qa187-path "eln")))
            (qa187-check (qa187-result source output) (cadr case))
            (qa187-check (file-exists-p output) nil)
            (qa187-check (list (fboundp 'qa187-valid-prefix) qa187-loaded
                      (featurep 'qa187-partial-feature)) '(nil nil nil)))))
      ;; Errors before the first complete form must also propagate.
      (let* ((source (qa187-source "(\n")) (output (qa187-path "eln")))
        (qa187-check (qa187-result source output) 'end-of-file)
        (qa187-check (file-exists-p output) nil))
      (let* ((source (qa187-source ")")) (output (qa187-path "eln")))
        (qa187-check (qa187-result source output) 'invalid-read-syntax)
        (qa187-check (file-exists-p output) nil))

      ;; A syntax failure must not overwrite an already existing output.
      (let* ((source (qa187-source "(defun qa187-valid-prefix () 7)\n(\n"))
             (output (qa187-path "eln")))
        (with-temp-file output (insert "existing artifact\n"))
        (qa187-check (qa187-result source output) 'end-of-file)
        (qa187-check (with-temp-buffer (insert-file-contents output) (buffer-string))
            "existing artifact\n"))

      ;; Cleanup still restores function cells, lexical binding and the
      ;; caller's lexical environment when a later read fails.
      (let ((before (symbol-function 'qa187-existing-macro))
            (binding lexical-binding)
            (captured 'caller))
        (let* ((source (qa187-source
                        "(defmacro qa187-existing-macro () ''temporary)\n(defmacro qa187-existing-macro () ''replacement)\n(defmacro qa187-new-macro () 9)\n(defvar qa187-declared)\n(defun qa187-valid-prefix () (qa187-existing-macro))\n(\n"))
               (output (qa187-path "eln")))
          (qa187-check (qa187-result source output) 'end-of-file)
          (qa187-check (eq before (symbol-function 'qa187-existing-macro)) t)
          (qa187-check (fboundp 'qa187-new-macro) nil)
          (qa187-check (list (boundp 'qa187-declared) (special-variable-p 'qa187-declared))
              '(nil nil))
          (qa187-check lexical-binding binding)
          (qa187-check (funcall (lambda () captured)) 'caller)
          (qa187-check (file-exists-p output) nil)))

      ;; Clean EOF is successful: exact EOF, trailing whitespace/comments,
      ;; strings containing delimiters, and Unicode forms.
      (dolist (tail '("" " \t\n" "; no newline ( ] \""
                      "\n; 日本語🙂 (\n; final comment"))
        (let* ((source (qa187-source
                        (concat "(defun qa187-good () \"日本語🙂;()[]\\\"\")" tail)))
               (output (qa187-path "eln")))
          (qa187-check (qa187-result source output) 'compiled)
          (qa187-check (file-exists-p output) t)
          (native-elisp-load output)
          (qa187-check (qa187-good) "日本語🙂;()[]\"")
          (qa187-check (native-comp-function-p (symbol-function 'qa187-good)) t)))
      ;; No implicit EOF exception is introduced for whitespace-only input.
      ;; Empty-file diagnostic subtypes differ, but neither accepts the file.
      (let* ((source (qa187-source "; only comments 日本語\n \t"))
             (output (qa187-path "eln")))
        (qa187-check (eq (qa187-result source output) 'compiled) nil)
        (qa187-check (file-exists-p output) nil))
      ;; A successful compile after failures still retains both Unicode and
      ;; ASCII forms and initializes only at load time.
      (let* ((source (qa187-source
                      "(setq qa187-loaded \"日本語\")\n(defun qa187-recovered () 19)\n(setq qa187-loaded (list qa187-loaded (qa187-recovered)))\n"))
             (output (qa187-path "eln")))
        (qa187-check (qa187-result source output) 'compiled)
        (qa187-check qa187-loaded nil)
        (native-elisp-load output)
        (qa187-check qa187-loaded '("日本語" 19))
        (qa187-check (native-comp-function-p (symbol-function 'qa187-recovered)) t)))
  (delete-directory qa187-directory t))

(princ (concat (qa--done) "\n"))
