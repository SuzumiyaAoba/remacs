;;; qa_compilation_quoted_tilde.el --- compilation quoted tilde regression -*- lexical-binding: t -*-

(provide 'comint)
(require 'compile)

(qa (compilation--unquote-shell-double-quoted-path "~/child") "./~/child")
(qa (compilation--unquote-shell-double-quoted-path "sub/~/child")
    "sub/~/child")

(let ((command (concat "cd " (string 34) "sub" (string 34) "/child; pwd")))
  (qa (compilation--unquote-shell-word
       (compilation--leading-cd-path command))
      "sub/child"))

(let ((command (concat "cd " (string 34) "foo" (string 92) (string 10)
                       "bar" (string 34) "; pwd")))
  (qa (compilation--unquote-shell-word
       (compilation--leading-cd-path command))
      "foobar"))

(let ((command (concat "cd " (string 34) "~/child" (string 34) "; pwd")))
  (qa (compilation--unquote-shell-word
       (compilation--leading-cd-path command))
      "./~/child"))

(qa (and (string-match-p "[\"'\\\\]" "\"sub\"/child") t) t)
(qa (not (string-match-p "[\"'\\\\]" "sub/child")) t)

(let ((process-environment (copy-sequence process-environment)))
  (setenv "COMPILATION_TILDE_TEST" "component")
  (qa (compilation--unquote-shell-word
       (compilation--leading-cd-path
        "cd \"$COMPILATION_TILDE_TEST/child\"; pwd"))
      "component/child")
  (qa (compilation--unquote-shell-word
       (compilation--leading-cd-path
        "cd '$COMPILATION_TILDE_TEST/child'; pwd"))
      "$COMPILATION_TILDE_TEST/child"))

(let* ((root (make-temp-file "compilation-quoted-tilde-" t))
       (default-directory (file-name-as-directory root))
       (literal-directory (expand-file-name "./~/child" default-directory)))
  (unwind-protect
      (progn
        (make-directory literal-directory t)
        (qa (expand-file-name
             (compilation--unquote-shell-double-quoted-path "~/child")
             default-directory)
            literal-directory)
        (qa (file-directory-p
             (expand-file-name
              (compilation--unquote-shell-double-quoted-path "~/child")
              default-directory))
            t))
    (delete-directory root t)))

(let* ((root (make-temp-file "compilation-concatenated-quote-" t))
       (child (expand-file-name "sub/child" root))
       (command (concat "cd " (string 34) "sub" (string 34)
                        "/child; pwd")))
  (unwind-protect
      (progn
        (make-directory child t)
        (with-temp-buffer
          (setq default-directory (file-name-as-directory root))
          (cd (compilation--unquote-shell-word
               (compilation--leading-cd-path command)))
          (qa (file-equal-p default-directory child) t)))
    (delete-directory root t)))

(princ (qa--done))
