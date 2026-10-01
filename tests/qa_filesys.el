;; filesys — file-name manipulation and file operations (POSIX/macOS).
;; GNU-31.1 quirks pinned below: directory-sep-char is unbound;
;; file-attribute-links/-status do not exist (use -link-number,
;; -status-change-time); expand-file-name keeps "//tmp" verbatim;
;; file-name-absolute-p is t for "~/x"; substitute-in-file-name expands
;; mid-path "~" ("/a/~/b" -> "~/b") but leaves a leading "~/" untouched;
;; file-local-copy on a local file returns nil; file-writable-p is t for a
;; nonexistent file in a writable directory; file-name-split "/" is
;; file-name-concat does not collapse "//"; file-name-join and
;; file-name-newer do not exist in GNU 31.1; backup-file-name-p returns
;; the index of the backup marker, not t; make-backup-file-name is a
;; plain "name~" append; file-name-sans-versions with KEEP-VERSION keeps
;; "foo~" whole; parse-colon-path yields directory names and nil for
;; empty segments; copy-directory copies the source AS the new name and
;; rejects an existing destination even with PARENTS; expand-file-name
;; leaves "~baduser" relative to the default directory.

;; == file-name-directory / file-name-nondirectory ==
(qa (file-name-directory "/a/b/c") "/a/b/")
(qa (file-name-directory "c") nil)
(qa (file-name-directory "/") "/")
(qa (file-name-directory "") nil)
(qa (file-name-directory "/a") "/")
(qa (file-name-nondirectory "/a/b/c") "c")
(qa (file-name-nondirectory "/a/b/") "")
(qa (file-name-nondirectory "/") "")
(qa (file-name-nondirectory "") "")

;; == extensions / base ==
(qa (file-name-sans-extension ".hidden") ".hidden")
(qa (file-name-sans-extension "a.b.c") "a.b")
(qa (file-name-sans-extension "noext") "noext")
(qa (file-name-sans-extension "/d/e.f/g") "/d/e.f/g")
(qa (file-name-sans-extension "/a/b.") "/a/b")
(qa (file-name-sans-extension "a.b/c") "a.b/c")
(qa (file-name-extension "a.b.c") "c")
(qa (file-name-extension "a.b.c" t) ".c")
(qa (file-name-extension ".c") nil)
(qa (file-name-extension "noext") nil)
(qa (file-name-extension "x.") "")
(qa (file-name-extension "/d.e/f") nil)
(qa (file-name-extension "/a.b/") nil)
(qa (file-name-base "/a/b/c.txt") "c")
(qa (file-name-base ".hidden") ".hidden")
(qa (file-name-base "/a/b/noext") "noext")
(qa (file-name-base "a.b/c.d") "c")
(qa (file-name-with-extension "a.b" "c") "a.c")
(qa (file-name-with-extension "a" ".c") "a.c")
(qa (file-name-with-extension "/d/e.f" "h") "/d/e.h")
(qa (file-name-with-extension "noext" "el") "noext.el")
(qa-err-data (file-name-with-extension "a.b" "") 'error '("Malformed extension: "))
(qa-err-data (file-name-with-extension "a" 5) 'wrong-type-argument '(stringp 5))

;; == expand-file-name / file-name-absolute-p / file-relative-name ==
(qa (expand-file-name "/a/b/../c") "/a/c")
(qa (expand-file-name "//tmp") "//tmp")
(qa (expand-file-name "/a//b") "/a/b")
(qa (expand-file-name "." "/x") "/x")
(qa (expand-file-name "" "/x") "/x")
(qa (expand-file-name "/" "/x") "/")
(qa (let ((default-directory "/qa/def/")) (expand-file-name "x")) "/qa/def/x")
(qa (let ((default-directory "/qa/def")) (expand-file-name "x")) "/qa/def/x")
(qa (file-name-absolute-p (expand-file-name "~/x")) t)
(qa (string-suffix-p "/x" (expand-file-name "~/x")) t)
(qa (let ((default-directory "/qa/def/"))
        (file-name-absolute-p (expand-file-name "~/x")))
    t)
(qa (file-name-absolute-p "/x") t)
(qa (file-name-absolute-p "x") nil)
(qa (file-name-absolute-p "~/x") t)
(qa (file-relative-name "/a/b/c" "/a/b") "c")
(qa (file-relative-name "/a/b/c" "/a/b/") "c")
(qa (file-relative-name "/a/b/c/d/e" "/a/b") "c/d/e")
(qa (file-relative-name "/a/b" "/a/b/c") "..")
(qa (file-relative-name "/a" "/a/b") "..")
(qa (file-relative-name "/a/b" "/x/y") "../../a/b")
(qa (file-relative-name "/a/b/c" "/a/b/c") ".")
(qa (file-relative-name "/a/b/c/" "/a/b") "c/")
(qa (file-relative-name "/a/bc" "/a/b") "../bc")
(qa (file-relative-name "/" "/a") "../")
(qa (file-relative-name "/a" "/") "a")
(qa (let ((default-directory "/qa/def/")) (file-relative-name "/qa/def/x")) "x")

;; == substitute-in-file-name / abbreviate-file-name ==
(qa (substitute-in-file-name "$NONEXISTENT_QAXYZ/x") "$NONEXISTENT_QAXYZ/x")
(qa (substitute-in-file-name "${NONEXISTENT_QAXYZ}/x") "${NONEXISTENT_QAXYZ}/x")
(qa (substitute-in-file-name "x${NONEXISTENT_QAXYZ}y") "x${NONEXISTENT_QAXYZ}y")
(qa (substitute-in-file-name "$$/x") "$/x")
(qa (substitute-in-file-name "~/qa-x") "~/qa-x")
(qa (substitute-in-file-name "/a/~/b") "~/b")
(qa (string-suffix-p "/qqx" (substitute-in-file-name "$HOME/qqx")) t)
(qa (file-name-absolute-p (substitute-in-file-name "$HOME/qqx")) t)
(qa (abbreviate-file-name "/etc/qa-nonexistent") "/etc/qa-nonexistent")
(qa (string-prefix-p "~/" (abbreviate-file-name (expand-file-name "~/qa-zz"))) t)

;; == directory names / concat / parent / split ==
(qa (directory-file-name "/a/") "/a")
(qa (directory-file-name "/") "/")
(qa (directory-file-name "") "")
(qa (directory-file-name "a/b//") "a/b")
(qa (file-name-as-directory "/a") "/a/")
(qa (file-name-as-directory "/a/") "/a/")
(qa (file-name-as-directory "/") "/")
(qa (file-name-as-directory "rel") "rel/")
(qa (file-name-as-directory "") "./")
(qa (file-name-concat "/a" "b" "c") "/a/b/c")
(qa (file-name-concat "/a/" "b") "/a/b")
(qa (file-name-concat "/a/" "/b") "/a//b")
(qa (file-name-concat "/a" "b/") "/a/b/")
(qa (file-name-concat "/a" "") "/a")
(qa (file-name-concat "a") "a")
(qa-err (file-name-concat) 'wrong-number-of-arguments)
(qa (file-name-parent-directory "/a/b/c") "/a/b/")
(qa (file-name-parent-directory "/a/b/") "/a/")
(qa (file-name-parent-directory "/a") "/")
(qa (file-name-parent-directory "/") nil)
(qa (file-name-parent-directory "rel/child") "rel/")
(qa (file-name-parent-directory "rel") "./")
(qa (file-name-split "a/b/c") '("a" "b" "c"))
(qa (file-name-split "/a/b") '("" "a" "b"))
(qa (file-name-split "a//b") '("a" "b"))
(qa (file-name-split "/a/") '("" "a" ""))
(qa (file-name-split "/") '("" "" ""))
(qa (file-name-split "//a") '("/" "a"))
(qa (file-name-split "") nil)

;; == quoting / remote / POSIX constants ==
(qa (file-name-quote "/a/b") "/:/a/b")
(qa (file-name-quote "a/b") "/:a/b")
(qa (file-name-quote "/:/a/b") "/:/a/b")
(qa (file-name-unquote "/:/a/b") "/a/b")
(qa (file-name-unquote "/a/b") "/a/b")
(qa (file-name-unquote "/:") "/")
(qa (file-name-quoted-p "/:/a") t)
(qa (file-name-quoted-p "/a") nil)
(qa (file-name-quoted-p "a/:b") nil)
(qa (file-name-quoted-p "/:" 'top) t)
(qa (file-remote-p "/a/b") nil)
(qa path-separator ":")
(qa null-device "/dev/null")
(qa (boundp 'directory-sep-char) nil)
(qa (fboundp 'file-attribute-link-number) t)
(qa (fboundp 'file-attribute-links) nil)
(qa (fboundp 'file-attribute-status-change-time) t)

;; == file-modes symbolic <-> numeric ==
(qa (file-modes-symbolic-to-number "a=r") #o444)
(qa (file-modes-symbolic-to-number "a=rx") #o555)
(qa (file-modes-symbolic-to-number "u=rwx,g=rx,o=rx") #o755)
(qa (file-modes-symbolic-to-number "u=rwx") #o700)
(qa (file-modes-symbolic-to-number "u+x" #o600) #o700)
(qa (file-modes-number-to-symbolic #o755) "-rwxr-xr-x")
(qa (file-modes-number-to-symbolic #o644) "-rw-r--r--")
(qa (file-modes-number-to-symbolic #o600) "-rw-------")
(qa (file-modes-number-to-symbolic #o755 ?d) "drwxr-xr-x")

;; == directory listings on a fixed tree ==
(let ((qafs-dir (make-temp-file "qafs-ls" t)))
  (unwind-protect
      (progn
        (with-temp-file (expand-file-name "a.txt" qafs-dir) (insert "abc"))
        (with-temp-file (expand-file-name "b.txt" qafs-dir) (insert "defgh"))
        (with-temp-file (expand-file-name "u.txt" qafs-dir) (insert "x"))
        (make-directory (expand-file-name "sub" qafs-dir))
        (with-temp-file (expand-file-name "sub/deep.txt" qafs-dir) (insert "d"))
        (make-symbolic-link "a.txt" (expand-file-name "lnk" qafs-dir))
        (make-symbolic-link "gone.txt" (expand-file-name "broken" qafs-dir))
        ;; directory-files / MATCH / FULL
        (qa (sort (directory-files qafs-dir nil "\\.txt$") #'string<)
            '("a.txt" "b.txt" "u.txt"))
        (qa (sort (mapcar #'file-name-nondirectory
                          (directory-files qafs-dir t "\\.txt$"))
                  #'string<)
            '("a.txt" "b.txt" "u.txt"))
        (qa (directory-files qafs-dir nil "nomatch") nil)
        (qa-err (directory-files "/nonexistent-qa-dir" nil "x") 'file-missing)
        ;; completions (sorted for determinism)
        (qa (sort (file-name-all-completions "" qafs-dir) #'string<)
            '("../" "./" "a.txt" "b.txt" "broken" "lnk" "sub/" "u.txt"))
        (qa (file-name-all-completions "a" qafs-dir) '("a.txt"))
        (qa (file-name-all-completions "zz" qafs-dir) nil)
        (qa (file-name-completion "a" qafs-dir) "a.txt")
        (qa (file-name-completion "sub" qafs-dir) "sub/")
        (qa (file-name-completion "zzz" qafs-dir) nil)
        (qa (file-name-completion "a" qafs-dir 'file-directory-p) nil)
        (qa (file-name-completion "sub" qafs-dir 'file-directory-p) "sub/")
        (qa-err (file-name-all-completions "" "/nonexistent-qa-dir") 'file-missing)
        ;; recursive listing, projected to names
        (qa (sort (mapcar #'file-name-nondirectory
                          (directory-files-recursively qafs-dir "\\.txt$"))
                  #'string<)
            '("a.txt" "b.txt" "deep.txt" "u.txt"))
        (qa (directory-files-recursively (expand-file-name "sub" qafs-dir) "zzz")
            nil)
        (qa-err (directory-files-recursively "/nonexistent-qa-dir" "x")
                'file-missing)
        ;; locate-file against a temp dir list
        (qa (file-name-nondirectory (locate-file "a.txt" (list qafs-dir)))
            "a.txt")
        (qa (locate-file "zzz.txt" (list qafs-dir)) nil)
        (qa (file-name-nondirectory (locate-file "a" (list qafs-dir) '(".txt")))
            "a.txt")
        (qa (file-name-nondirectory
             (locate-file "a.txt" (list qafs-dir) nil #'file-readable-p))
            "a.txt")
        (qa (locate-file "a.txt" (list qafs-dir) nil #'file-executable-p) nil)
        ;; symlink surface
        (qa (file-symlink-p (expand-file-name "lnk" qafs-dir)) "a.txt")
        (qa (file-regular-p (expand-file-name "lnk" qafs-dir)) t)
        (qa (file-symlink-p (expand-file-name "broken" qafs-dir)) "gone.txt")
        (qa (file-exists-p (expand-file-name "broken" qafs-dir)) nil)
        (qa (file-equal-p (expand-file-name "a.txt" qafs-dir)
                          (expand-file-name "lnk" qafs-dir))
            t)
        (qa (file-name-nondirectory
             (file-truename (expand-file-name "lnk" qafs-dir)))
            "a.txt"))
    (delete-directory qafs-dir t)))

;; == file operations under a fresh temp dir ==
(let* ((qafs-root (make-temp-file "qafs-ops" t))
       (qafs-a (expand-file-name "a.txt" qafs-root))
       (qafs-b (expand-file-name "b.txt" qafs-root))
       (qafs-u (expand-file-name "u.txt" qafs-root))
       (qafs-miss (expand-file-name "nope" qafs-root)))
  (unwind-protect
      (progn
        (with-temp-file qafs-a (insert "abc"))
        (with-temp-file qafs-b (insert "defgh"))
        ;; predicates
        (qa (file-exists-p qafs-a) t)
        (qa (file-regular-p qafs-a) t)
        (qa (file-directory-p qafs-a) nil)
        (qa (file-directory-p qafs-root) t)
        (qa (file-directory-p (file-name-as-directory qafs-root)) t)
        (qa (file-directory-p (concat qafs-a "/")) nil)
        (qa (file-symlink-p qafs-a) nil)
        (qa (file-readable-p qafs-a) t)
        (qa (file-writable-p qafs-a) t)
        (qa (file-executable-p qafs-a) nil)
        (qa (file-accessible-directory-p qafs-root) t)
        (qa (file-accessible-directory-p qafs-a) nil)
        (qa (file-remote-p qafs-a) nil)
        (qa (file-exists-p qafs-root) t)
        (qa (file-exists-p qafs-miss) nil)
        (qa (file-regular-p qafs-miss) nil)
        (qa (file-readable-p qafs-miss) nil)
        (qa (file-writable-p qafs-miss) t)
        (qa (file-accessible-directory-p qafs-miss) nil)
        (qa (file-modes qafs-miss) nil)
        (qa (file-locked-p qafs-a) nil)
        (qa (file-locked-p qafs-miss) nil)
        (qa-err-data (file-exists-p 5) 'wrong-type-argument '(stringp 5))
        ;; file-attributes shape and accessors
        (qa (car (file-attributes qafs-a)) nil)
        (qa (car (file-attributes qafs-root)) t)
        (qa (nth 7 (file-attributes qafs-a)) 3)
        (qa (nth 7 (file-attributes qafs-a 'string)) 3)
        (qa (file-attribute-size (file-attributes qafs-a)) 3)
        (qa (file-attribute-type (file-attributes qafs-a)) nil)
        (qa (file-attribute-type (file-attributes qafs-root)) t)
        (qa (file-attribute-link-number (file-attributes qafs-a)) 1)
        (qa (file-attribute-collect (file-attributes qafs-a) 'type 'size)
            '(nil 3))
        (qa (consp (file-attribute-modification-time (file-attributes qafs-a)))
            t)
        (qa (consp (file-attribute-access-time (file-attributes qafs-a))) t)
        (qa (stringp (file-attribute-modes (file-attributes qafs-a))) t)
        (qa (file-attributes qafs-miss) nil)
        (qa (file-attributes qafs-miss 'integer) nil)
        ;; set-file-modes / file-modes roundtrip
        (qa (set-file-modes qafs-a #o600) nil)
        (qa (file-modes qafs-a) #o600)
        (qa (logand #o777 (file-modes qafs-a)) #o600)
        (qa (file-attribute-modes (file-attributes qafs-a)) "-rw-------")
        (qa (nth 8 (file-attributes qafs-a)) "-rw-------")
        (qa (set-file-modes qafs-a #o640) nil)
        (qa (nth 8 (file-attributes qafs-a)) "-rw-r-----")
        (qa (set-file-modes qafs-a #o755) nil)
        (qa (file-executable-p qafs-a) t)
        (qa (set-file-modes qafs-a #o644) nil)
        (qa (file-executable-p qafs-a) nil)
        ;; set-file-times + file-newer-than-file-p, deterministic mtimes
        (qa (set-file-times qafs-a '(0 0)) t)
        (qa (time-equal-p (file-attribute-modification-time
                           (file-attributes qafs-a))
                          '(0 0))
            t)
        (qa (set-file-times qafs-b '(1000 0)) t)
        (qa (file-newer-than-file-p qafs-b qafs-a) t)
        (qa (file-newer-than-file-p qafs-a qafs-b) nil)
        (qa (file-newer-than-file-p qafs-a qafs-a) nil)
        (qa (file-newer-than-file-p qafs-a qafs-miss) t)
        (qa (file-newer-than-file-p qafs-miss qafs-a) nil)
        ;; file-equal-p / file-in-directory-p / file-truename
        (qa (file-equal-p qafs-a qafs-a) t)
        (qa (file-equal-p qafs-a qafs-b) nil)
        (qa (file-equal-p qafs-a qafs-miss) nil)
        (qa (file-in-directory-p qafs-a qafs-root) t)
        (qa (file-in-directory-p "/definitely-not-qa" qafs-root) nil)
        (qa (file-name-absolute-p (file-truename qafs-a)) t)
        (qa (file-exists-p (file-truename qafs-a)) t)
        (qa (file-equal-p (file-truename qafs-a) qafs-a) t)
        ;; file-local-copy on a local file returns nil in GNU
        (qa (file-local-copy qafs-a) nil)
        ;; copy / rename / delete
        (qa (copy-file qafs-a (expand-file-name "copy.txt" qafs-root)) nil)
        (qa (with-temp-buffer
              (insert-file-contents (expand-file-name "copy.txt" qafs-root))
              (buffer-string))
            "abc")
        (qa-err-data (copy-file qafs-a (expand-file-name "copy.txt" qafs-root))
                     'file-already-exists
                     (list "File already exists"
                           (expand-file-name "copy.txt" qafs-root)))
        (qa (copy-file qafs-b (expand-file-name "copy.txt" qafs-root) t) nil)
        (qa (with-temp-buffer
              (insert-file-contents (expand-file-name "copy.txt" qafs-root))
              (buffer-string))
            "defgh")
        (qa (rename-file qafs-b (expand-file-name "renamed.txt" qafs-root)) nil)
        (qa (file-exists-p qafs-b) nil)
        (qa (with-temp-buffer
              (insert-file-contents (expand-file-name "renamed.txt" qafs-root))
              (buffer-string))
            "defgh")
        (qa-err-data (rename-file qafs-a (expand-file-name "copy.txt" qafs-root))
                     'file-already-exists
                     (list "File already exists"
                           (expand-file-name "copy.txt" qafs-root)))
        (qa-err (rename-file qafs-miss (expand-file-name "zz.txt" qafs-root))
                'file-missing)
        (qa (delete-file (expand-file-name "renamed.txt" qafs-root)) nil)
        (qa (file-exists-p (expand-file-name "renamed.txt" qafs-root)) nil)
        (qa (delete-file qafs-miss) nil)
        ;; write-region / insert-file-contents roundtrip
        (qa (write-region "zz" nil (expand-file-name "w.txt" qafs-root)
                          nil 'nomsg)
            nil)
        (qa (write-region "XY" nil (expand-file-name "w.txt" qafs-root)
                          'append 'nomsg)
            nil)
        (qa (with-temp-buffer
              (insert-file-contents (expand-file-name "w.txt" qafs-root))
              (buffer-string))
            "zzXY")
        (qa (with-temp-buffer
              (insert "hello")
              (write-region 2 5 (expand-file-name "pos.txt" qafs-root)
                            nil 'nomsg))
            nil)
        (qa (with-temp-buffer
              (insert-file-contents (expand-file-name "pos.txt" qafs-root))
              (buffer-string))
            "ell")
        (qa-err-data (write-region "x" nil (expand-file-name "no-dir/f"
                                                           qafs-root))
                     'file-missing
                     (list "Opening output file" "No such file or directory"
                           (expand-file-name "no-dir/f" qafs-root)))
        ;; insert-file-contents return value (list NAME SIZE-in-chars) + range
        (qa (cdr (with-temp-buffer (insert-file-contents qafs-a))) '(3))
        (qa (file-name-nondirectory
             (car (with-temp-buffer (insert-file-contents qafs-a))))
            "a.txt")
        (qa (with-temp-buffer (insert-file-contents qafs-a nil 1 3)
              (buffer-string))
            "bc")
        (qa (with-temp-buffer (insert-file-contents qafs-a nil 0 0)
              (buffer-string))
            "")
        (qa-err (insert-file-contents qafs-miss) 'file-missing)
        (qa-err (insert-file-contents qafs-root) 'file-error)
        ;; insert-file-contents-literally: no decoding
        (with-temp-file qafs-u (insert "é"))
        (qa (file-attribute-size (file-attributes qafs-u)) 2)
        (qa (with-temp-buffer (insert-file-contents qafs-u) (buffer-string))
            "é")
        (qa (with-temp-buffer (insert-file-contents qafs-u)
              (length (buffer-string)))
            1)
        (qa (cdr (with-temp-buffer (insert-file-contents qafs-u))) '(1))
        (qa (with-temp-buffer (insert-file-contents-literally qafs-u)
              (length (buffer-string)))
            2)
        (qa (cdr (with-temp-buffer (insert-file-contents-literally qafs-u)))
            '(2))
        (qa (with-temp-buffer
              (set-buffer-multibyte nil)
              (insert-file-contents-literally qafs-u)
              (buffer-string))
            "\303\251")
        ;; make-directory / delete-directory
        (qa (make-directory (expand-file-name "d1/d2/d3" qafs-root) t) nil)
        (qa (file-directory-p (expand-file-name "d1/d2/d3" qafs-root)) t)
        (qa (make-directory (expand-file-name "d1/d2/d3" qafs-root) t) t)
        (qa-err-data (make-directory (expand-file-name "d1" qafs-root))
                     'file-already-exists
                     (list "File exists" (expand-file-name "d1" qafs-root)))
        (qa (delete-directory (expand-file-name "d1/d2/d3" qafs-root)) nil)
        (qa-err-data (delete-directory (expand-file-name "d1" qafs-root))
                     'file-error
                     (list "Removing directory" "Directory not empty"
                           (expand-file-name "d1" qafs-root)))
        (qa (delete-directory (expand-file-name "d1" qafs-root) t) nil)
        (qa (file-exists-p (expand-file-name "d1" qafs-root)) nil))
    (delete-directory qafs-root t)))

;; == make-temp-name / make-temp-file ==
(qa (string-prefix-p "qapx" (file-name-nondirectory (make-temp-name "qapx")))
    t)
(qa (equal (make-temp-name "qapx") (make-temp-name "qapx")) nil)
(qa (file-exists-p (make-temp-name "qapx")) nil)
(let ((qafs-tf (make-temp-file "qafs-mtf")))
  (unwind-protect
      (progn
        (qa (file-exists-p qafs-tf) t)
        (qa (file-regular-p qafs-tf) t)
        (qa (file-directory-p qafs-tf) nil)
        (qa (file-attribute-size (file-attributes qafs-tf)) 0))
    (delete-file qafs-tf)))
(let ((qafs-tf (make-temp-file "qafs-mtft" nil nil "prelude")))
  (unwind-protect
      (qa (with-temp-buffer (insert-file-contents qafs-tf) (buffer-string))
          "prelude")
    (delete-file qafs-tf)))
(let ((qafs-td (make-temp-file "qafs-mtd" t)))
  (unwind-protect
      (qa (file-directory-p qafs-td) t)
    (delete-directory qafs-td)))


;; == expand-file-name / file-relative-name corner cases ==
(qa (expand-file-name "../y" "/a/b") "/a/y")
(qa (expand-file-name "x" "/qa/def") "/qa/def/x")
(qa (expand-file-name "./x" "/qa/def/") "/qa/def/x")
(qa (expand-file-name "/a/./b") "/a/b")
(qa (expand-file-name "a/../b" "/x") "/x/b")
(qa (expand-file-name "/x" "/ignored") "/x")
(qa (expand-file-name "x/" "/qa/def") "/qa/def/x/")
(qa-err (expand-file-name nil) 'wrong-type-argument)
;; "~baduser" does not expand: it stays relative to DEFAULT
(qa (expand-file-name "~nosuchuser-qa-x/f" "/qa-dd/") "/qa-dd/~nosuchuser-qa-x/f")
(qa (expand-file-name "~nosuchuser-qa-x" "/qa-dd/") "/qa-dd/~nosuchuser-qa-x")
(qa (substitute-in-file-name "~nosuchuser-qa-x/f") "~nosuchuser-qa-x/f")
(qa (file-relative-name "/a/b" "/a/b/") ".")
(qa (file-relative-name "/a/b/" "/a/b/c") "../")
(qa (let ((default-directory "/d1/d2/")) (file-relative-name "x" "/d1")) "d2/x")
(qa (let ((default-directory "/d1/d2/")) (file-relative-name "x" "/d1/d2")) "x")

;; == nondirectory/directory/extension extra corners ==
(qa (file-name-nondirectory "~") "~")
(qa (file-name-nondirectory "/a/b/.hidden") ".hidden")
(qa (file-name-directory "~/x") "~/")
(qa (file-name-directory "~user/x") "~user/")
(qa (file-name-extension "/a/b/.hidden") nil)
(qa (file-name-sans-extension "~/x.y") "~/x")
(qa (file-name-sans-extension ".emacs.d/init.el") ".emacs.d/init")

;; == file-name-concat / file-name-split extra corners ==
(qa (file-name-concat "/" "a") "/a")
(qa (file-name-concat "" "a") "a")
(qa (file-name-concat nil "a") "a")
(qa (file-name-concat "/a" nil "b") "/a/b")
(qa (file-name-concat "/a" "b" "c/d") "/a/b/c/d")
(qa (file-name-split "~/a") '("~" "a"))
(qa (file-name-split "a/") '("a" ""))
(qa (file-name-split "a/b/") '("a" "b" ""))

;; == quoting extra corners ==
(qa (file-name-quote "") "/:")
(qa (file-name-quote "/:") "/:")
(qa (file-name-quoted-p "/:" 'top) t)
(qa (file-name-quoted-p "/:/x" 'top) t)
(qa (file-name-unquote "") "")
(qa-err (file-name-unquote nil) 'wrong-type-argument)

;; == substitute-in-file-name env vars (deterministic let-bound env) ==
(qa (let ((process-environment (cons "QAFSV=/qa-val-xyz" process-environment)))
      (substitute-in-file-name "$QAFSV/sub"))
    "/qa-val-xyz/sub")
(qa (let ((process-environment (cons "QAFSV=/qa-val-xyz" process-environment)))
      (substitute-in-file-name "pre$QAFSV"))
    "pre/qa-val-xyz")
(qa (let ((process-environment (cons "QAFSV=v" process-environment)))
      (substitute-in-file-name "${QAFSV}post"))
    "vpost")
(qa (substitute-in-file-name "/a/$QA-no-var-zzz/b") "/a/$QA-no-var-zzz/b")
(qa (substitute-in-file-name "x~") "x~")

;; == directory-abbrev-alist / abbreviate-file-name ==
(qa (let ((directory-abbrev-alist '(("\\`/qa-abbrev-zz" . "/tmp"))))
      (abbreviate-file-name "/qa-abbrev-zz/x"))
    "/tmp/x")
(qa (let ((directory-abbrev-alist nil))
      (abbreviate-file-name "/etc/zz-qa"))
    "/etc/zz-qa")

;; == backup/version file-name helpers ==
(qa (file-name-sans-versions "foo.el.~1~") "foo.el")
(qa (file-name-sans-versions "foo.el~") "foo.el")
(qa (file-name-sans-versions "foo.~42~") "foo")
(qa (file-name-sans-versions "/a/b/foo.~2~") "/a/b/foo")
(qa (file-name-sans-versions "foo.~2~bak") "foo.~2~bak")
(qa (file-name-sans-versions "foo..~2~") "foo.")
(qa (file-name-sans-versions "foo.~2a~") "foo")
(qa (file-name-sans-versions "foo") "foo")
(qa (file-name-sans-versions ".~2~") "")
(qa (file-name-sans-versions "foo.~2~/bar") "foo.~2~/bar")
(qa (file-name-sans-versions "foo.~2~~") "foo.~2~")
;; KEEP-VERSION non-nil keeps the "foo~" backup form untouched
(qa (file-name-sans-versions "foo~" t) "foo~")
(qa (file-name-sans-versions "foo.~2~" t) "foo.~2~")
(qa (file-name-sans-versions "foo~" nil) "foo")
(qa (backup-file-name-p "foo~") 3)
(qa (backup-file-name-p "foo") nil)
(qa (backup-file-name-p "/a/b~") 4)
(qa (backup-file-name-p "foo.~3~") 6)
(qa (backup-file-name-p "foo.bak") nil)
(qa (backup-file-name-p "a~~") 2)
(qa (backup-file-name-p "~/b~") 3)
(qa (backup-file-name-p "foo.#") nil)
(qa (make-backup-file-name "foo") "foo~")
(qa (make-backup-file-name "/a/b.txt") "/a/b.txt~")

;; == parse-colon-path ==
(qa (parse-colon-path "/a:/b/c") '("/a/" "/b/c/"))
(qa (parse-colon-path ":/x") '(nil "/x/"))
(qa (parse-colon-path "") '(nil))
(qa (parse-colon-path nil) nil)
(qa (parse-colon-path "/a:") '("/a/" nil))
(qa (parse-colon-path ":") '(nil nil))
(qa (parse-colon-path "a::b") '("a/" nil "b/"))

;; == functions absent in GNU 31.1 ==
(qa (fboundp 'file-name-join) nil)
(qa (fboundp 'file-name-newer) nil)
(qa (fboundp 'file-name-undirectory) nil)

;; == temporary-file-directory ==
(qa (stringp temporary-file-directory) t)
(qa (file-name-absolute-p temporary-file-directory) t)
(qa (file-directory-p temporary-file-directory) t)

;; == copy-directory / dir symlink / visiting-replace under temp tree ==
(let* ((qafs-cd (make-temp-file "qafs-cd" t))
       (qafs-src (expand-file-name "srcdir" qafs-cd))
       (qafs-dst (expand-file-name "dstdir" qafs-cd)))
  (unwind-protect
      (progn
        (make-directory qafs-src)
        (with-temp-file (expand-file-name "x.txt" qafs-src) (insert "x"))
        (make-directory (expand-file-name "inner" qafs-src))
        (with-temp-file (expand-file-name "inner/y.txt" qafs-src) (insert "yy"))
        ;; copy-directory copies SOURCE as NEWNAME recursively; returns nil
        (qa (copy-directory qafs-src qafs-dst) nil)
        (qa (file-directory-p qafs-dst) t)
        (qa (file-regular-p (expand-file-name "x.txt" qafs-dst)) t)
        (qa (file-directory-p (expand-file-name "inner" qafs-dst)) t)
        (qa (file-regular-p (expand-file-name "inner/y.txt" qafs-dst)) t)
        (qa (with-temp-buffer
              (insert-file-contents (expand-file-name "inner/y.txt" qafs-dst))
              (buffer-string))
            "yy")
        ;; existing destination rejected even with PARENTS=t
        (qa-err-data (copy-directory qafs-src qafs-dst)
                     'file-already-exists
                     (list "File exists" qafs-dst))
        (qa-err-data (copy-directory qafs-src qafs-dst nil nil t)
                     'file-already-exists
                     (list "File exists" qafs-dst))
        ;; missing source
        (qa-err (copy-directory (expand-file-name "nosrc" qafs-cd)
                                (expand-file-name "dst2" qafs-cd))
                'file-missing)
        ;; symlink to a directory: dir predicates follow the link,
        ;; file-attributes reports the target as type
        (make-symbolic-link qafs-src (expand-file-name "dlink" qafs-cd))
        (qa (file-directory-p (expand-file-name "dlink" qafs-cd)) t)
        (qa (file-accessible-directory-p (expand-file-name "dlink" qafs-cd)) t)
        (qa (file-regular-p (expand-file-name "dlink" qafs-cd)) nil)
        (qa (file-regular-p qafs-src) nil)
        (qa (file-symlink-p (expand-file-name "dlink" qafs-cd)) qafs-src)
        (qa (file-attribute-type
             (file-attributes (expand-file-name "dlink" qafs-cd)))
            qafs-src)
        ;; insert-file-contents REPLACE visits: fine on empty buffer,
        ;; error on non-empty
        (qa (with-temp-buffer
              (insert-file-contents
               (expand-file-name "inner/y.txt" qafs-src) t)
              (buffer-string))
            "yy")
        (qa-err (with-temp-buffer
                  (insert "X")
                  (insert-file-contents
                   (expand-file-name "inner/y.txt" qafs-src) t))
                'error)
        ;; file-truename of a nonexistent path just returns it expanded
        (qa (file-truename "/qa-no-such-xyz/f") "/qa-no-such-xyz/f")
        ;; file-newer-than-file-p with both files missing
        (qa (file-newer-than-file-p "/qa-miss1-xyz" "/qa-miss2-xyz") nil)
        ;; executable-find for a command that cannot exist
        (qa (executable-find "definitely-no-such-cmd-qa-xyz") nil)
        ;; file-attributes on symlink: car is the link target
        (qa (car (file-attributes (expand-file-name "dlink" qafs-cd)))
            qafs-src))
    (delete-directory qafs-cd t)))

(princ (qa--done))
