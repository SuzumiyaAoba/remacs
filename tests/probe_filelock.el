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
(princ "filelock-ok")
