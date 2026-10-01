;; Glob/directory/symlink domain — GNU 31.1 pins (verified).
;;
;; Setup: /tmp/qagd/ tree built at load time, removed at the end.
;; GNU truths pinned below:
;;   - file-expand-wildcards returns ABSOLUTE names when the pattern is
;;     absolute; RELATIVE names when the pattern is relative (resolved
;;     against default-directory).
;;   - `?' matches one char; the ".txt" suffix filters out f2.log.
;;   - directory-files order is readdir-dependent — sort for determinism.
;;   - insert-directory writes ls-style output into the buffer; returns nil.
;;   - file-truename resolves symlinks ("/tmp" → "/private/tmp" on macOS).
;;   - file-symlink-p returns the TARGET NAME string, not t.
;;   - make-symbolic-link OK-IF-ALREADY-EXISTS=t overwrites silently.
;;   - file-relative-name produces ../.. forms, never an absolute fallback.
;;   - locate-dominating-file treats NAME literally (wildcards don't match).

;; == setup ==
(make-directory "/tmp/qagd/sub" t)
(with-temp-file "/tmp/qagd/f1.txt" (insert "1"))
(with-temp-file "/tmp/qagd/f2.log" (insert "2"))
(with-temp-file "/tmp/qagd/fA.txt" (insert "A"))
(with-temp-file "/tmp/qagd/sub/f3.txt" (insert "3"))

;; == file-expand-wildcards ==
;; *.txt matches only .txt files
(qa (file-expand-wildcards "/tmp/qagd/*.txt")
    '("/tmp/qagd/f1.txt" "/tmp/qagd/fA.txt"))
;; FULL=t keeps absolute names
(qa (file-expand-wildcards "/tmp/qagd/*.txt" t)
    '("/tmp/qagd/f1.txt" "/tmp/qagd/fA.txt"))
;; no match → nil
(qa (file-expand-wildcards "/tmp/qagd/*.nomatch") nil)
;; ? matches one char — with ".txt" suffix only .txt files survive
(qa (file-expand-wildcards "/tmp/qagd/f?.txt" t)
    '("/tmp/qagd/f1.txt" "/tmp/qagd/fA.txt"))
;; [abc] char class
(qa (file-expand-wildcards "/tmp/qagd/f[12].txt" t)
    '("/tmp/qagd/f1.txt"))
;; "*" matches everything incl dirs
(qa (sort (copy-sequence (file-expand-wildcards "/tmp/qagd/*")) #'string<)
    '("/tmp/qagd/f1.txt" "/tmp/qagd/f2.log" "/tmp/qagd/fA.txt" "/tmp/qagd/sub"))
;; nonexistent dir prefix → nil
(qa (file-expand-wildcards "/tmp/qagd-nope/*.txt") nil)
;; relative wildcard returns RELATIVE names (vs default-directory)
(qa (let ((default-directory "/tmp/qagd/"))
      (file-expand-wildcards "*.txt"))
    '("f1.txt" "fA.txt"))
;; relative pattern + FULL=t → absolute
(qa (let ((default-directory "/tmp/qagd/"))
      (file-expand-wildcards "*.txt" t))
    '("/tmp/qagd/f1.txt" "/tmp/qagd/fA.txt"))

;; == directory-files (supplement qa_filesys: MATCH/FULL/COUNT args) ==
;; GNU's NOSORT result is cons-accumulated reversed readdir order.
(qa (directory-files "/tmp/qagd" nil "\\.txt\\'" t)
    '("f1.txt" "fA.txt"))
;; sort-for-determinism works under both (sorting is caller-side)
(qa (sort (copy-sequence (directory-files "/tmp/qagd" nil "f[12]" t))
          #'string<)
    '("f1.txt" "f2.log"))
;; COUNT arg caps the result
(qa (length (directory-files "/tmp/qagd" nil nil nil 2)) 2)
;; FULL arg → absolute
(qa (car (directory-files "/tmp/qagd" t "f1\\.txt\\'")) "/tmp/qagd/f1.txt")

;; == directory-files-and-attributes ==
(qa (caar (directory-files-and-attributes "/tmp/qagd" nil "f1\\.txt\\'" t))
    "f1.txt")
(let ((e (assoc "f1.txt" (directory-files-and-attributes "/tmp/qagd"))))
  (qa (listp e) t))

;; == insert-directory ==
;; writes ls-style output; returns nil
(qa (with-temp-buffer
      (insert-directory "/tmp/qagd" "-a" nil t)
      (numberp (string-match "f1\\.txt" (buffer-string))))
    t)
;; -l gives long format (total line present)
(qa (with-temp-buffer
      (insert-directory "/tmp/qagd" "-l" nil t)
      (string-prefix-p "total" (buffer-string)))
    t)
;; a file name arg lists that file
(qa (with-temp-buffer
      (insert-directory "/tmp/qagd/f1.txt" "-l" nil t)
      (numberp (string-match "f1\\.txt" (buffer-string))))
    t)
;; return value is nil
(qa (with-temp-buffer (insert-directory "/tmp/qagd" "-a" nil t)) nil)

;; == locate-dominating-file ==
;; finds the file by walking up
(qa (locate-dominating-file "/tmp/qagd/sub" "f1.txt") "/tmp/qagd/")
;; file in the starting dir
(qa (locate-dominating-file "/tmp/qagd" "f1.txt") "/tmp/qagd/")
;; nonexistent → nil
(qa (locate-dominating-file "/tmp/qagd" "no-such-xyz") nil)
;; NAME can be a predicate
(qa (locate-dominating-file "/tmp/qagd/sub"
                            (lambda (dir) (file-exists-p (concat dir "f1.txt"))))
    "/tmp/qagd/")
;; wildcard NAME is a literal filename, not a pattern
(qa (locate-dominating-file "/tmp/qagd/sub" "f?.txt") nil)

;; == file-truename / file-symlink-p / file-equal-p ==
;; /tmp is a symlink on macOS → resolves to /private/tmp
(qa (file-truename "/tmp/qagd/f1.txt") "/private/tmp/qagd/f1.txt")
;; nonexistent → resolves the dir anyway
(qa (file-truename "/tmp/qagd/nope.txt") "/private/tmp/qagd/nope.txt")
;; relative name resolves against default-directory
(qa (let ((default-directory "/tmp/qagd/"))
      (file-truename "f1.txt"))
    "/private/tmp/qagd/f1.txt")
;; symlink: file-symlink-p returns the raw target string
(make-symbolic-link "f1.txt" "/tmp/qagd/sl.txt" t)
(qa (file-symlink-p "/tmp/qagd/sl.txt") "f1.txt")
(qa (file-symlink-p "/tmp/qagd/f1.txt") nil)
;; file-truename follows the link
(qa (file-truename "/tmp/qagd/sl.txt") "/private/tmp/qagd/f1.txt")
;; absolute symlink target
(make-symbolic-link "/tmp/qagd/f2.log" "/tmp/qagd/sl2.log" t)
(qa (file-symlink-p "/tmp/qagd/sl2.log") "/tmp/qagd/f2.log")
(qa (file-truename "/tmp/qagd/sl2.log") "/private/tmp/qagd/f2.log")
;; file-equal-p
(qa (file-equal-p "/tmp/qagd" "/tmp/qagd/") t)
(qa (file-equal-p "/tmp/qagd" "/tmp/qagd/sub") nil)
;; file-in-directory-p
(qa (file-in-directory-p "/tmp/qagd/f1.txt" "/tmp/qagd") t)
(qa (file-in-directory-p "/tmp/qagd/sub/f3.txt" "/tmp/qagd") t)
(qa (file-in-directory-p "/tmp/qagd/f1.txt" "/tmp/qagd/sub") nil)
(qa (file-in-directory-p "/tmp/elsewhere" "/tmp/qagd") nil)

;; == file-name-concat ==
(qa (file-name-concat "/tmp/qagd" "x" "y.txt") "/tmp/qagd/x/y.txt")
;; second arg absolute still concatenates (GNU pin: double slash)
(qa (file-name-concat "/tmp/qagd/" "/x") "/tmp/qagd//x")
;; one arg → as-is
(qa (file-name-concat "/tmp/qagd") "/tmp/qagd")

;; == file-name-* accessors ==
(qa (file-name-as-directory "/tmp/qagd") "/tmp/qagd/")
(qa (file-name-as-directory "/tmp/qagd/") "/tmp/qagd/")
(qa (directory-file-name "/tmp/qagd/") "/tmp/qagd")
(qa (file-name-directory "/tmp/qagd/f1.txt") "/tmp/qagd/")
;; a directory name's file-name-directory is itself
(qa (file-name-directory "/tmp/qagd/") "/tmp/qagd/")
(qa (file-name-nondirectory "/tmp/qagd/f1.txt") "f1.txt")
;; .hidden has NO extension
(qa (file-name-extension "/tmp/qagd/f1.txt") "txt")
(qa (file-name-extension "/tmp/qagd/f1") nil)
(qa (file-name-extension "/tmp/qagd/.txt") nil)
;; sans-extension strips the last .EXT
(qa (file-name-sans-extension "/tmp/qagd/f1.txt") "/tmp/qagd/f1")
(qa (file-name-sans-extension "/tmp/qagd/f1.tar.gz") "/tmp/qagd/f1.tar")

;; == file-relative-name ==
(qa (file-relative-name "/tmp/qagd/sub/f3.txt" "/tmp/qagd") "sub/f3.txt")
(qa (file-relative-name "/tmp/qagd/f1.txt" "/tmp/qagd/") "f1.txt")
;; outside the dir → ../.. relative form (NOT absolute fallback)
(qa (file-relative-name "/etc/hosts" "/tmp/qagd") "../../etc/hosts")
;; .. traversal
(qa (file-relative-name "/tmp/other/f.txt" "/tmp/qagd") "../other/f.txt")

;; == file-name-absolute-p / expand-file-name anchors ==
(qa (file-name-absolute-p "/tmp/qagd") t)
(qa (file-name-absolute-p "relative/x") nil)
(qa (file-name-absolute-p "~/x") t)
(qa (expand-file-name "x" "/tmp/qagd/") "/tmp/qagd/x")
(qa (expand-file-name "../f1.txt" "/tmp/qagd/sub/") "/tmp/qagd/f1.txt")
;; ~ expands to HOME
(qa (string-prefix-p (expand-file-name "~") (expand-file-name "~/x.txt")) t)
;; / as base + relative → /x
(qa (expand-file-name "x" "/") "/x")
;; expand-file-name on an absolute name ignores DIR
(qa (expand-file-name "/abs" "/tmp/qagd") "/abs")

;; == abbreviate-file-name ==
;; under HOME it abbreviates to ~/
(qa (string-prefix-p "~" (abbreviate-file-name (expand-file-name "~/qagd_ab")))
    t)
;; outside HOME unchanged
(qa (abbreviate-file-name "/tmp/qagd/f1.txt") "/tmp/qagd/f1.txt")

;; == file-newer-than-file-p ==
;; fA.txt created after f1.txt — same-second mtimes make this
;; unreliable, so pin via set-file-times for determinism
(progn
  (with-temp-file "/tmp/qagd/old.txt" (insert "o"))
  (set-file-times "/tmp/qagd/old.txt" '(0 1))
  (qa (file-newer-than-file-p "/tmp/qagd/f1.txt" "/tmp/qagd/old.txt") t)
  (qa (file-newer-than-file-p "/tmp/qagd/old.txt" "/tmp/qagd/f1.txt") nil)
  ;; same file → nil
  (qa (file-newer-than-file-p "/tmp/qagd/f1.txt" "/tmp/qagd/f1.txt") nil))

;; == directory-empty-p ==
(qa (directory-empty-p "/tmp/qagd") nil)
(make-directory "/tmp/qagd_empty" t)
(qa (directory-empty-p "/tmp/qagd_empty") t)
(qa (directory-empty-p "/tmp/qagd-nope") nil)

;; == make-symbolic-link OK-IF-ALREADY-EXISTS ==
;; t → second call overwrites silently (no error)
(progn
  (make-symbolic-link "f1.txt" "/tmp/qagd/sl3.txt" t)
  (qa (make-symbolic-link "zzz" "/tmp/qagd/sl3.txt" t) nil)
  (qa (file-symlink-p "/tmp/qagd/sl3.txt") "zzz")
  ;; nil → file-already-exists
  (qa-err (make-symbolic-link "zzz" "/tmp/qagd/sl3.txt")
          'file-already-exists))

;; == directory creation/deletion extras ==
(progn
  (make-directory "/tmp/qagd_mk/a/b/c" t)
  (qa (file-directory-p "/tmp/qagd_mk/a/b/c") t)
  (delete-directory "/tmp/qagd_mk" t))
;; delete-directory non-recursive on non-empty → error
(qa-err (delete-directory "/tmp/qagd") 'file-error)
;; delete-empty-directory on empty → nil
(progn
  (make-directory "/tmp/qagd_del" t)
  (qa (delete-directory "/tmp/qagd_del") nil)
  (qa (file-directory-p "/tmp/qagd_del") nil))

;; == copy-directory ==
(progn
  (copy-directory "/tmp/qagd" "/tmp/qagd_copy" nil t t)
  (qa (file-exists-p "/tmp/qagd_copy/f1.txt") t)
  (qa (file-exists-p "/tmp/qagd_copy/sub/f3.txt") t)
  (delete-directory "/tmp/qagd_copy" t))

;; == rename-file ==
(progn
  (with-temp-file "/tmp/qagd_mv.txt" (insert "m"))
  (rename-file "/tmp/qagd_mv.txt" "/tmp/qagd_mv2.txt")
  (qa (file-exists-p "/tmp/qagd_mv.txt") nil)
  (qa (file-exists-p "/tmp/qagd_mv2.txt") t)
  (delete-file "/tmp/qagd_mv2.txt"))

;; == cleanup ==
(dolist (f '("/tmp/qagd/sl.txt" "/tmp/qagd/sl2.log" "/tmp/qagd/sl3.txt"
             "/tmp/qagd/f1.txt" "/tmp/qagd/f2.log" "/tmp/qagd/fA.txt"
             "/tmp/qagd/old.txt" "/tmp/qagd/sub/f3.txt"))
  (when (file-exists-p f) (delete-file f)))
(when (file-directory-p "/tmp/qagd_empty") (delete-directory "/tmp/qagd_empty"))
(delete-directory "/tmp/qagd" t)

(princ (qa--done))
