;;; qa_compilation_quoted_tilde.el --- compilation quoted tilde regression -*- lexical-binding: t -*-

(require 'compile)

(qa (compilation--unquote-shell-double-quoted-path "~/child") "./~/child")
(qa (compilation--unquote-shell-double-quoted-path "sub/~/child")
    "sub/~/child")

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

(princ (qa--done))
