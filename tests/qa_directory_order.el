;;; -*- lexical-binding: nil; -*-
;; Issue #23: stream-order selection, completion order and prefix protocol.
;; Expectations are also checked by GNU Emacs 31.1 on the same fixture.

(defun qa23-take (items count)
  (let (result)
    (while (and items (> count 0))
      (push (car items) result)
      (setq items (cdr items) count (1- count)))
    (nreverse result)))

(let ((qa23-dir (make-temp-file "qa23-directory-" t))
      (completion-ignored-extensions nil)
      (completion-regexp-list nil)
      (completion-ignore-case nil)
      (case-fold-search nil))
  (unwind-protect
      (progn
        (dolist (name '("utf8.txt" "utf8bom.txt" "utf8crlf.txt" "utf16le.txt"
                        "utf16be.txt" "sjis.txt" "b-first" "b-next" ".hidden"))
          (with-temp-file (expand-file-name name qa23-dir) (insert "data")))
        (make-directory (expand-file-name "sub" qa23-dir))
        (make-symbolic-link "sub" (expand-file-name "dir-link" qa23-dir))
        (make-symbolic-link "gone" (expand-file-name "broken" qa23-dir))

        ;; Exact t, ambiguous unchanged prefix string, extension and no-match.
        (qa (file-name-completion "utf8" qa23-dir) "utf8")
        (qa (file-name-completion "utf" qa23-dir nil) "utf")
        (qa (file-name-completion "" qa23-dir) "")
        (qa (file-name-completion "b" qa23-dir
                                  (lambda (name) (string-prefix-p "b-" name))) "b-")
        (qa (file-name-completion "utf8.txt" qa23-dir) t)
        (qa (file-name-completion "nosuch" qa23-dir) nil)
        (qa (file-name-completion "sub" qa23-dir) "sub/")
        (qa (file-name-completion "dir-link" qa23-dir) "dir-link/")
        (qa (file-name-completion "sub" qa23-dir #'file-directory-p) "sub/")

        ;; All-completions includes dot entries in GNU too. Its order is
        ;; cons accumulation of readdir, as for unsorted directory-files.
        (qa (sort (file-name-all-completions "." qa23-dir) #'string<)
            '("../" "./" ".hidden"))
        (qa (file-name-all-completions "" qa23-dir)
            (mapcar (lambda (name)
                      (concat name (if (file-directory-p (expand-file-name name qa23-dir))
                                       "/" "")))
                    (directory-files qa23-dir nil nil t)))
        (qa (file-name-all-completions "u" qa23-dir)
            (delq nil (mapcar (lambda (name) (and (string-prefix-p "u" name) name))
                             (directory-files qa23-dir nil nil t))))
        (qa (let ((calls nil))
              (file-name-completion "utf" qa23-dir
                                    (lambda (name) (push name calls) nil))
              (reverse calls))
            (delq nil (mapcar (lambda (name) (and (string-prefix-p "utf" name) name))
                             (reverse (directory-files qa23-dir nil nil t)))))

        ;; COUNT selects the FIRST matching stream entries. Only then do
        ;; NOSORT and the optional sort determine their returned order.
        (dolist (qa23-count '(0 1 2 3 5 20))
          (qa (directory-files qa23-dir nil nil t qa23-count)
              (reverse (qa23-take (reverse (directory-files qa23-dir nil nil t))
                                  qa23-count)))
          (qa (directory-files qa23-dir nil nil nil qa23-count)
              (sort (qa23-take (reverse (directory-files qa23-dir nil nil t))
                               qa23-count) #'string<))
          (qa (mapcar #'file-name-nondirectory
                      (directory-files qa23-dir t nil t qa23-count))
              (reverse (qa23-take (reverse (directory-files qa23-dir nil nil t))
                                  qa23-count)))
          (qa (directory-files qa23-dir nil "\\.txt\\'" t qa23-count)
              (reverse (qa23-take
                        (delq nil (mapcar (lambda (name)
                                            (and (string-suffix-p ".txt" name) name))
                                          (reverse (directory-files qa23-dir nil nil t))))
                        qa23-count)))
          (qa (mapcar #'car (directory-files-and-attributes
                            qa23-dir nil nil t nil qa23-count))
              (directory-files qa23-dir nil nil t qa23-count)))

        ;; Attribute values come from stat on the actual same directory,
        ;; not a guessed directory-size constant from another fixture.
        (qa (cdr (assoc "." (directory-files-and-attributes qa23-dir)))
            (file-attributes (concat (file-name-as-directory qa23-dir) ".")))
        (qa (cdr (assoc "utf8.txt" (directory-files-and-attributes qa23-dir)))
            (file-attributes (expand-file-name "utf8.txt" qa23-dir)))
        (qa (mapcar #'car (directory-files-and-attributes qa23-dir nil nil nil))
            (directory-files qa23-dir))
        (qa-err-data (directory-files qa23-dir nil nil nil -1)
                     'wrong-type-argument '(wholenump -1)))
    (delete-directory qa23-dir t)))

;; Dot entries must not prevent completing the sole actual candidate.
(let ((qa23-dir (make-temp-file "qa23-single-" t))
      (completion-ignored-extensions nil))
  (unwind-protect
      (progn
        (with-temp-file (expand-file-name "only.txt" qa23-dir) (insert "x"))
        (qa (file-name-completion "" qa23-dir) "only.txt")
        (qa (file-name-completion "only.txt" qa23-dir) t)
        (qa (sort (file-name-all-completions "" qa23-dir) #'string<)
            '("../" "./" "only.txt")))
    (delete-directory qa23-dir t)))

(princ (qa--done)) (terpri)
