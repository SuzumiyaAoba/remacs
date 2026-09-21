;; Visited-file modtime + file-lock primitives, GNU-compatible.
;; Self-asserting: any mismatch signals `error'.
(let* ((dir (make-temp-file "remacs-lock" t))
       (f (expand-file-name "exists.txt" dir))
       (missing (expand-file-name "absent.txt" dir))
       (lname (expand-file-name ".#exists.txt" dir)))
  (unwind-protect
      (progn
        (with-temp-file f (insert "hello\n"))
        ;; --- visited-file-modtime ---
        (let ((b (find-file-noselect f)))
          (unwind-protect
              (with-current-buffer b
                (cl-assert (consp (visited-file-modtime)))
                (cl-assert (verify-visited-file-modtime b))
                (set-visited-file-modtime '(100 200 300 400))
                ;; GNU truncates to ns: ps 400 -> 0.
                (cl-assert (equal (visited-file-modtime) '(100 200 300 0)))
                (set-visited-file-modtime -1)
                (cl-assert (eq (visited-file-modtime) -1))
                (cl-assert (not (verify-visited-file-modtime b)))
                (set-visited-file-modtime 0)
                (cl-assert (eq (visited-file-modtime) 0))
                (cl-assert (verify-visited-file-modtime b))
                ;; Other fixnums are out of range.
                (cl-assert (eq (condition-case e (progn (set-visited-file-modtime 5) 'ok)
                                (error (car e)))
                               'args-out-of-range))
                (clear-visited-file-modtime)
                (cl-assert (eq (visited-file-modtime) 0))
                (set-visited-file-modtime nil)
                (cl-assert (consp (visited-file-modtime)))
                (cl-assert (verify-visited-file-modtime b))
                ;; Size changed on disk -> verify fails.
                (with-temp-file f (insert "changed!"))
                (cl-assert (not (verify-visited-file-modtime b))))
            (kill-buffer b)))
        ;; Missing file: -1 and verified.
        (let ((b (find-file-noselect missing)))
          (unwind-protect
              (with-current-buffer b
                (cl-assert (eq (visited-file-modtime) -1))
                (cl-assert (verify-visited-file-modtime b))
                (cl-assert (eq (condition-case e (progn (set-visited-file-modtime 7) 'ok)
                                  (error (car e)))
                          'args-out-of-range)))
            (kill-buffer b)))
        ;; Non-file buffer: 0 / t / wrong-type-argument.
        (with-current-buffer (get-buffer-create " *fl-nofile")
          (cl-assert (eq (visited-file-modtime) 0))
          (cl-assert (verify-visited-file-modtime (current-buffer)))
          (cl-assert (eq (condition-case e (progn (set-visited-file-modtime nil) 'ok)
                            (error (car e)))
                     'wrong-type-argument)))
        ;; --- locking ---
        (let ((b (find-file-noselect f)))
          (unwind-protect
              (with-current-buffer b
                (cl-assert (not (file-locked-p f)))
                ;; Insertion locks the visited file (GNU lock_file).
                (insert "x")
                (cl-assert (eq (file-locked-p f) t))
                (cl-assert (file-symlink-p lname))
                ;; Relocking our own lock is a no-op.
                (cl-assert (null (lock-buffer)))
                ;; Clearing modified releases the lock.
                (set-buffer-modified-p nil)
                (cl-assert (not (file-locked-p f)))
                (cl-assert (not (file-symlink-p lname)))
                ;; lock-buffer on unmodified buffer is a no-op.
                (cl-assert (null (lock-buffer)))
                (cl-assert (not (file-locked-p f)))
                ;; Explicit unlock after modification.
                (insert "y")
                (cl-assert (eq (file-locked-p f) t))
                (unlock-buffer)
                (cl-assert (not (file-locked-p f)))
                (set-buffer-modified-p nil))
            (kill-buffer b)))
        ;; Foreign lock: file-locked-p returns the user name; a
        ;; dead-pid same-host lock is stale => nil.
        (make-symbolic-link "otheruser@otherhost.999999" lname t)
        (cl-assert (equal (file-locked-p f) "otheruser"))
        ;; set-buffer-modified-p t on a foreign-locked file signals
        ;; file-locked (GNU prepare_to_modify_buffer -> lock_file).
        (let ((b (find-file-noselect f)))
          (unwind-protect
              (with-current-buffer b
                (cl-assert (eq (condition-case e (progn (set-buffer-modified-p t) 'ok)
                                    (error (car e)))
                               'file-locked))
                ;; The failed set-buffer-modified-p left the buffer
                ;; unmodified, so lock-buffer is still a no-op.
                (cl-assert (eq (condition-case e (progn (lock-buffer) 'ok)
                                 (error (car e)))
                               'ok)))
            (kill-buffer b)))
        (delete-file lname)
        ;; ask-user-about-lock signals file-locked in batch.
        (cl-assert (eq (condition-case e
                           (progn (ask-user-about-lock f "someone") 'ok)
                         (error (car e)))
                   'file-locked))
        ;; create-lockfiles nil => no lock on modification.  GNU's
        ;; `let' binds in parallel, so the binding must be established
        ;; before find-file-noselect.
        (let ((create-lockfiles nil))
          (let ((b2 (find-file-noselect f)))
            (unwind-protect
                (with-current-buffer b2
                  (insert "z")
                  (cl-assert (not (file-symlink-p lname)))
                  (cl-assert (null (lock-buffer)))
                  (set-buffer-modified-p nil))
              (kill-buffer b2)))))
    (delete-directory dir t)))
;; file-equal-p / file-remote-p / file-system-info /
;; set-default-file-modes.
(let* ((dir (make-temp-file "remacs-fsinfo" t))
       (f1 (expand-file-name "a.txt" dir))
       (f2 (expand-file-name "b.txt" dir))
       (f3 (expand-file-name "c.txt" dir)))
  (unwind-protect
      (progn
        (with-temp-file f1 (insert "x"))
        (make-symbolic-link f1 f2 t) ;; symlink: GNU file-equal-p stats
        (with-temp-file f3 (insert "y"))
        (cl-assert (file-equal-p f1 f1))
        (cl-assert (file-equal-p f1 f2))
        (cl-assert (not (file-equal-p f1 f3)))
        (cl-assert (not (file-equal-p f1 (expand-file-name "nope" dir))))
        (cl-assert (not (file-remote-p f1)))
        (cl-assert (not (file-remote-p "~/x")))
        (cl-assert (equal (file-remote-p "/ssh:user@host:/p")
                          "/ssh:user@host:"))
        (cl-assert (equal (file-remote-p "/ssh:user@host:/p" 'method) "ssh"))
        (cl-assert (equal (file-remote-p "/ssh:user@host:/p" 'user) "user"))
        (cl-assert (equal (file-remote-p "/ssh:user@host:/p" 'host) "host"))
        (cl-assert (equal (file-remote-p "/ssh:user@host:/p" 'localname) "/p"))
        (cl-assert (equal (file-remote-p "/scp:host:/p" 'user) nil))
        (let ((fsi (file-system-info dir)))
          (cl-assert (and (consp fsi) (= (length fsi) 3)
                          (cl-every #'integerp fsi))))
        (cl-assert (null (file-system-info "/no/such/dir/remacs-xyz")))
        ;; set-default-file-modes returns nil and sets the subr value + umask.
        (let ((orig-modes (default-file-modes)))
          (cl-assert (null (set-default-file-modes #o640)))
          (cl-assert (= (default-file-modes) #o640))
          (let ((f4 (expand-file-name "m.txt" dir)))
            (with-temp-file f4 (insert "z"))
            (cl-assert (= (file-modes f4) #o640)))
          ;; umask is process-global; restore the original.
          (set-default-file-modes orig-modes)))
    (delete-directory dir t)))

;; copy-directory: GNU files.el port.
(let* ((base (make-temp-file "remacs-cd" t))
       (src (expand-file-name "src" base)))
  (unwind-protect
      (progn
        (make-directory (expand-file-name "inner" src) t)
        (with-temp-file (expand-file-name "top.txt" src) (insert "top\n"))
        (with-temp-file (expand-file-name "inner/deep.txt" src)
          (insert "deep\n"))
        (set-file-times (expand-file-name "top.txt" src)
                        '(1000 2000 3000 4000))
        (make-symbolic-link "top.txt" (expand-file-name "lnk" src) t)
        ;; plain copy, recursively.
        (cl-assert (null (copy-directory src (expand-file-name "dst" base))))
        (cl-assert (file-exists-p (expand-file-name "dst/top.txt" base)))
        (cl-assert (file-exists-p (expand-file-name "dst/inner/deep.txt" base)))
        ;; keep-time preserves mtimes at ps precision.
        (copy-directory src (expand-file-name "kt" base) t)
        (cl-assert (equal (file-attribute-modification-time
                           (file-attributes
                            (expand-file-name "kt/top.txt" base)))
                          '(1000 2000 3000 4000)))
        ;; symlinks are recreated, not dereferenced.
        (cl-assert (equal (file-symlink-p (expand-file-name "dst/lnk" base))
                          "top.txt"))
        ;; copy into an existing directory nests by basename.
        (let ((existing (expand-file-name "exist" base)))
          (make-directory existing)
          (copy-directory src (file-name-as-directory existing))
          (cl-assert (file-exists-p
                      (expand-file-name "src/top.txt" existing))))
        ;; copy-contents copies children directly.
        (cl-assert (null (copy-directory src (expand-file-name "cc" base)
                                       nil nil t)))
        (cl-assert (file-exists-p (expand-file-name "cc/top.txt" base)))
        ;; parents creates intermediate dirs.
        (copy-directory src (expand-file-name "a/b/c" base) nil t)
        (cl-assert (file-exists-p (expand-file-name "a/b/c/top.txt" base)))
        ;; copying into own subdirectory is an error.
        (cl-assert (eq 'error
                       (car (condition-case e
                                (copy-directory src (expand-file-name "sub" src))
                              (error e)))))
        ;; nonexistent source: GNU errors on the make-directory of an
        ;; existing target, or file-error on missing source.
        (cl-assert (consp (condition-case e
                              (copy-directory (expand-file-name "nope" base)
                                              (expand-file-name "d2" base))
                            (error e)))))
    (delete-directory base t)))
(princ "filelock-ok")
