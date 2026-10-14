;; buffer<->file — visiting, inserting, writing, saving, locking,
;; auto-save names, modtime bookkeeping and buffer naming.
;; GNU 31.1 -Q --batch ground truth (all verified by probing):
;; - `find-file-wildcards' defaults to t, but `find-file'/
;;   `find-file-noselect' do NOT expand a wildcard name unless the
;;   WILDCARDS argument is non-nil — "dir/*.txt" is visited literally.
;; - `buffer-file-truename' and `buffer-auto-save-file-name' are
;;   VARIABLES, not functions (truename set at visit; the auto-save
;;   name stays nil until a real auto-save happens).
;; - `auto-save-mode' is a FUNCTION; there is no such variable.
;; - `visited-file-modtime' returns a Lisp timestamp list like
;;   (27323 4456 263013 904000), NOT an integer; after
;;   `clear-visited-file-modtime' it is integer 0.
;; - `set-visited-file-modtime' rejects a bare integer with
;;   args-out-of-range; a list like (0 0) becomes (0 0 0 0).
;; - `write-region', `write-file', `append-to-file', `save-buffer',
;;   `unlock-buffer' and `set-file-modes' all return nil.
;; - write-region APPEND as an integer OVERWRITES at that offset:
;;   writing "ZZ" at offset 0 of "STRAP" yields "ZZRAP".
;; - write-region VISIT: t sets buffer-file-name and clears the
;;   modified flag; a string becomes the new buffer-file-name;
;;   'quiet only records the file modtime (verify-visited-file-modtime
;;   t afterwards) without setting buffer-file-name.
;; - `require-final-newline' defaults to t, so write-file puts a
;;   trailing newline in the FILE while the buffer stays "wf".
;; - `save-some-buffers' with a non-nil arg returns t when it saved
;;   something, nil when every visited buffer was unmodified.
;; - `save-buffer' on a buffer with no file prompts "File to save in:"
;;   and signals end-of-file in batch.
;; - `file-locked-p' returns t for a file locked by THIS Emacs,
;;   the login name (string) for a foreign lock, nil for a lock
;;   symlink owned by our user from a different pid, nil for a
;;   nonexistent file.  The lock file .#NAME is a dangling symlink:
;;   file-exists-p is nil, file-symlink-p is a string.
;; - A foreign lock makes the first buffer-modifying operation signal
;;   `file-locked'; `ask-user-about-lock' signals `file-locked' in
;;   batch ("Cannot resolve lock conflict in batch mode").
;; - `auto-save-file-name-p' returns an index (0), not t; it rejects
;;   names containing a slash ("/a/#b#" -> nil).
;; - `file-visited-newer-p' does not exist in GNU 31.1.
;; - insert-file-contents-literally on a UTF-8 BOM file keeps raw
;;   bytes: in a multibyte buffer they appear as chars
;;   4194287/4194235/4194239 (eight-bit range); the non-literal insert
;;   strips the signature and yields "x".
;; - `find-file-visit-truename' is inert for find-file-noselect:
;;   buffer-file-name keeps the symlink path either way.  But
;;   visiting a symlink whose truename target is already visited
;;   returns the SAME buffer (alias detection).

;; Silence "Wrote ..."/auto-save message noise.
(setq inhibit-message t)

;; == fboundp / default values ==
(qa (fboundp 'find-file-existing) t)
(qa (fboundp 'find-file-literally) t)
(qa (fboundp 'file-visited-newer-p) nil)
(qa (fboundp 'delete-auto-save-file-if-necessary) t)
(qa find-file-wildcards t)
(qa find-file-visit-truename nil)
(qa auto-save-default t)
(qa (boundp 'buffer-file-truename) t)
(qa (boundp 'buffer-auto-save-file-name) t)

;; == write-region argument validation and preservation ==
(let* ((dir (make-temp-file "qa157-" t))
       (file (expand-file-name "existing" dir))
       (missing (expand-file-name "missing" dir)))
  (unwind-protect
      (with-temp-buffer
        (insert "abcdef")
        (write-region "OLD" nil file nil 'quiet)
        (qa (write-region 5 2 file nil 'quiet) nil)
        (qa (with-temp-buffer (insert-file-contents file) (buffer-string)) "bcd")
        (qa (write-region 5 2 missing nil 'quiet) nil)
        (qa (with-temp-buffer (insert-file-contents missing) (buffer-string)) "bcd")
        (qa (write-region 5 2 file t 'quiet) nil)
        (qa (with-temp-buffer (insert-file-contents file) (buffer-string)) "bcdbcd")
        (qa (write-region 2 2 file nil 'quiet) nil)
        (qa (nth 7 (file-attributes file)) 0)
        (write-region "KEEP" nil file nil 'quiet)
        (dolist (lock '(excl 17 t (x)))
          (qa-err-data (write-region nil nil file nil 'quiet lock)
                       'wrong-type-argument (list 'stringp lock)))
        (qa (with-temp-buffer (insert-file-contents file) (buffer-string)) "KEEP")
        (when (file-exists-p missing) (delete-file missing))
        (qa-err-data (write-region "BAD" nil missing nil 'quiet 'excl)
                     'wrong-type-argument '(stringp excl))
        (qa (file-exists-p missing) nil)
        (qa-err-data (write-region 1 nil file nil 'quiet)
                     'wrong-type-argument '(integer-or-marker-p nil))
        (qa-err-data (write-region 'bad 2 17 nil 'quiet 'bad-lock)
                     'wrong-type-argument '(integer-or-marker-p bad))
        (qa-err-data (write-region 1 2 17 nil 'quiet 'bad-lock)
                     'wrong-type-argument '(stringp 17))
        (qa-err-data (write-region nil nil file nil 'quiet 'bad-lock 'excl)
                     'wrong-type-argument '(stringp bad-lock))
        (qa (write-region "STR" 'ignored file nil 'quiet nil) nil)
        (qa (with-temp-buffer (insert-file-contents file) (buffer-string)) "STR")
        (narrow-to-region 2 5)
        (qa (write-region nil 'ignored file nil 'quiet) nil)
        (qa (with-temp-buffer (insert-file-contents file) (buffer-string)) "abcdef")
        (qa (write-region 5 2 file nil 'quiet) nil)
        (qa-err-data (write-region 1 2 file nil 'quiet)
                     'args-out-of-range (list (current-buffer) 1 2))
        (qa-err-data (write-region 6 2 file nil 'quiet)
                     'args-out-of-range (list (current-buffer) 6 2))
        (let ((first (copy-marker 2)) (last (copy-marker 5)))
          (qa (write-region last first file nil 'quiet) nil)
          (qa (with-temp-buffer (insert-file-contents file) (buffer-string)) "bcd")
          (set-marker first nil)
          (qa-err-data (write-region first last file nil 'quiet)
                       'error '("Marker does not point anywhere"))))
    (delete-directory dir t)))

;; == write-region / append-to-file / write-file / file-modes ==
(let* ((qabf-dir (make-temp-file "qabf-wr" t))
       (qabf-f (expand-file-name "w.txt" qabf-dir)))
  ;; write-region returns nil; START/END are buffer positions
  (qa (with-temp-buffer
        (insert "abcdefghij")
        (write-region 2 5 qabf-f nil 'quiet))
      nil)
  (qa (with-temp-buffer (insert-file-contents qabf-f) (buffer-string)) "bcd")
  ;; START as a string writes the string itself
  (qa (progn (write-region "STR" nil qabf-f nil 'quiet)
             (with-temp-buffer (insert-file-contents qabf-f)
                               (buffer-string)))
      "STR")
  ;; APPEND t appends
  (with-temp-buffer
    (write-region "AP" nil qabf-f t 'quiet)
    (qa (with-temp-buffer (insert-file-contents qabf-f) (buffer-string))
        "STRAP"))
  ;; APPEND as integer overwrites at that byte offset
  (with-temp-buffer
    (write-region "ZZ" nil qabf-f 0 'quiet)
    (qa (with-temp-buffer (insert-file-contents qabf-f) (buffer-string))
        "ZZRAP"))
  ;; nil START/END writes the whole buffer
  (qa (with-temp-buffer (insert "xyz") (write-region nil nil qabf-f nil 'quiet))
      nil)
  ;; VISIT t: buffer-file-name set, modified flag cleared
  (qa (with-temp-buffer
        (insert "v")
        (write-region nil nil qabf-f nil t)
        (list (buffer-file-name) (buffer-modified-p)))
      (list qabf-f nil))
  ;; VISIT 'quiet: no buffer-file-name, but the file modtime is remembered
  (qa (with-temp-buffer
        (insert "v")
        (write-region nil nil qabf-f nil 'quiet)
        buffer-file-name)
      nil)
  (qa (with-temp-buffer
        (insert "v")
        (write-region nil nil qabf-f nil 'quiet)
        (verify-visited-file-modtime))
      t)
  ;; VISIT as a string becomes the new visited name
  (let ((qabf-f2 (expand-file-name "w2.txt" qabf-dir)))
    (qa (with-temp-buffer
          (insert "v")
          (write-region nil nil qabf-f nil qabf-f2)
          (list (buffer-file-name) (buffer-modified-p)))
        (list qabf-f2 nil)))
  ;; write-region into a directory signals file-error
  (qa-err (with-temp-buffer
            (insert "x") (write-region nil nil qabf-dir nil 'quiet))
          'file-error)
  ;; write-file: returns nil, visits the file, renames the buffer.
  ;; require-final-newline is t, so a newline is added to the buffer
  ;; before writing: the buffer (and file) end up "wf\n".
  (let ((qabf-f3 (expand-file-name "w3.txt" qabf-dir)))
    (qa (with-temp-buffer (insert "wf") (write-file qabf-f3)) nil)
    (qa (with-temp-buffer
          (insert "wf")
          (write-file qabf-f3)
          (list (buffer-file-name) (buffer-name) (buffer-modified-p)
                (buffer-string)))
        (list qabf-f3 "w3.txt" nil "wf\n"))
    (qa (with-temp-buffer (insert-file-contents qabf-f3) (buffer-string))
        "wf\n"))
  ;; append-to-file: nil return; END is exclusive; creates the file
  (let ((qabf-f4 (expand-file-name "ap.txt" qabf-dir)))
    (qa (with-temp-buffer
          (insert "abcdef")
          (append-to-file 2 4 qabf-f4)
          (append-to-file 1 2 qabf-f4)
          (with-temp-buffer (insert-file-contents qabf-f4) (buffer-string)))
        "bca"))
  (let ((qabf-f5 (expand-file-name "ap2.txt" qabf-dir)))
    (qa (with-temp-buffer
          (insert "XY")
          (list (append-to-file nil nil qabf-f5)
                (with-temp-buffer (insert-file-contents qabf-f5)
                                  (buffer-string))))
        (list nil "XY")))
  ;; file-modes / set-file-modes octal roundtrip
  (qa (logand (file-modes qabf-f) #o777) #o644)
  (qa (set-file-modes qabf-f #o600) nil)
  (qa (file-modes qabf-f) #o600)
  (set-file-modes qabf-f #o644)
  (qa (file-modes (expand-file-name "no.txt" qabf-dir)) nil)
  (qa-err (set-file-modes (expand-file-name "no.txt" qabf-dir) #o644)
          'file-missing)
  (delete-directory qabf-dir t))

;; == insert-file-contents / insert-file-contents-literally ==
(let* ((qabf-dir (make-temp-file "qabf-ins" t))
       (qabf-f (expand-file-name "ins.txt" qabf-dir)))
  (write-region "0123456789" nil qabf-f nil 'quiet)
  ;; return value is (FILENAME CHARS-INSERTED)
  (qa (with-temp-buffer (insert-file-contents qabf-f)) (list qabf-f 10))
  (qa (with-temp-buffer (insert-file-contents qabf-f) (buffer-string))
      "0123456789")
  ;; BEG/END are 0-based byte offsets; 3 chars inserted
  (qa (with-temp-buffer
        (list (insert-file-contents qabf-f nil 2 5) (buffer-string)))
      (list (list qabf-f 3) "234"))
  ;; REPLACE non-nil replaces the buffer content
  (qa (with-temp-buffer
        (insert "abcdef")
        (list (insert-file-contents qabf-f nil 1 4 t) (buffer-string)))
      (list (list qabf-f 3) "123"))
  ;; markers after the insertion point move with the text
  (qa (with-temp-buffer
        (insert "AB")
        (goto-char (point-min))
        (let ((m (copy-marker 2)))
          (insert-file-contents qabf-f nil 0 1)
          (list (buffer-string) (marker-position m))))
      '("0AB" 3))
  ;; VISIT non-nil: buffer-file-name is set as a side effect
  (qa (with-temp-buffer
        (list (insert-file-contents qabf-f t) (buffer-file-name)
              (buffer-string)))
      (list (list qabf-f 10) qabf-f "0123456789"))
  (qa-err (with-temp-buffer (insert-file-contents "/no/such-qafile-xyz"))
          'file-missing)
  (qa-err (with-temp-buffer (insert-file-contents qabf-dir)) 'file-error)
  ;; literally: UTF-8 BOM bytes stay raw in a multibyte buffer
  (let ((qabf-lf (expand-file-name "bom.txt" qabf-dir)))
    (write-region "\xEF\xBB\xBFx" nil qabf-lf nil 'quiet)
    (qa (with-temp-buffer (insert-file-contents-literally qabf-lf))
        (list qabf-lf 4))
    (qa (with-temp-buffer
          (insert-file-contents-literally qabf-lf)
          (string-to-list (buffer-string)))
        '(4194287 4194235 4194239 120))
    (qa (with-temp-buffer
             (insert-file-contents-literally qabf-lf)
             (set-buffer-multibyte nil)
             (string-to-list (buffer-string)))
           '(239 187 191 120))
    ;; non-literal: utf-8 signature is stripped, "x" remains
    (qa (with-temp-buffer (insert-file-contents qabf-lf) (buffer-string)) "x")
    (qa-err (with-temp-buffer (insert-file-contents-literally "/no/qa-xyz"))
            'file-missing))
  (delete-directory qabf-dir t))

;; == find-file / find-file-noselect ==
(let* ((qabf-dir (make-temp-file "qabf-ff" t))
       (qabf-f1 (expand-file-name "a.txt" qabf-dir))
       (qabf-f2 (expand-file-name "b.txt" qabf-dir))
       (qabf-elf (expand-file-name "x.el" qabf-dir)))
  (write-region "hello\n" nil qabf-f1 nil 'quiet)
  (write-region "world\n" nil qabf-f2 nil 'quiet)
  (write-region "(+ 1 2)" nil qabf-elf nil 'quiet)
  ;; noselect returns the visiting buffer
  (let ((b (find-file-noselect qabf-f1)))
    (qa (bufferp b) t)
    (qa (buffer-name b) "a.txt")
    (qa (with-current-buffer b (buffer-string)) "hello\n")
    (qa (buffer-file-name b) qabf-f1)
    ;; auto-mode picked from the .txt name
    (qa (with-current-buffer b major-mode) 'text-mode)
    (qa (buffer-modified-p b) nil)
    ;; same file -> same buffer (idempotent)
    (qa-same (find-file-noselect qabf-f1) b)
    ;; buffer-file-truename resolves the /var -> /private/var symlink
    (qa (buffer-local-value 'buffer-file-truename b) (file-truename qabf-f1))
    ;; buffer-file-name is also a FUNCTION of one buffer arg
    (qa (buffer-file-name b) qabf-f1)
    (kill-buffer b))
  ;; .el -> emacs-lisp-mode via auto-mode-alist
  (let ((b (find-file-noselect qabf-elf)))
    (qa (with-current-buffer b major-mode) 'emacs-lisp-mode)
    (kill-buffer b))
  ;; nonexistent file: empty visiting buffer, buffer-file-name set
  (let* ((nf (expand-file-name "nonexist.txt" qabf-dir))
         (b (find-file-noselect nf)))
    (qa (buffer-name b) "nonexist.txt")
    (qa (buffer-file-name b) nf)
    (qa (with-current-buffer b (buffer-string)) "")
    (kill-buffer b))
  ;; directory: visited in dired-mode; buffer-file-name is nil there
  (let ((b (find-file-noselect qabf-dir)))
    (qa (with-current-buffer b major-mode) 'dired-mode)
    (qa (buffer-file-name b) nil)
    (kill-buffer b))
  ;; find-file in batch switches the current buffer
  (let ((cur (current-buffer)))
    (find-file qabf-f2)
    (qa (buffer-name) "b.txt")
    (qa-same (current-buffer) (find-buffer-visiting qabf-f2))
    (kill-buffer (current-buffer))
    (set-buffer cur))
  ;; find-file-literally visits without conversion
  (let ((b (find-file-literally qabf-f1)))
    (qa (buffer-name b) "a.txt")
    (qa (with-current-buffer b (buffer-string)) "hello\n")
    (kill-buffer b))
  ;; find-file-existing: ok on existing, plain `error' otherwise
  (qa (buffer-name (find-file-existing qabf-f1)) "a.txt")
  (kill-buffer (find-buffer-visiting qabf-f1))
  (qa-err (find-file-existing (expand-file-name "no.txt" qabf-dir)) 'error)
  ;; find-buffer-visiting nil for an unvisited file
  (qa (find-buffer-visiting (expand-file-name "zzz.txt" qabf-dir)) nil)
  ;; wildcards: NOT expanded by default — a literal "*.txt" buffer
  (let ((b (find-file-noselect (expand-file-name "*.txt" qabf-dir))))
    (qa (buffer-name b) "*.txt")
    (kill-buffer b))
  ;; ...but the WILDCARDS arg expands when find-file-wildcards is t.
  ;; Only files that actually exist on disk match.
  (let ((bufs (find-file-noselect (expand-file-name "*.txt" qabf-dir)
                                  nil nil t)))
    (qa (and (listp bufs) (mapcar #'buffer-name bufs)) '("a.txt" "b.txt"))
    (if (listp bufs) (mapc #'kill-buffer bufs) (kill-buffer bufs)))
  ;; find-file-wildcards nil disables expansion even with WILDCARDS arg
  (let ((find-file-wildcards nil))
    (let ((b (find-file-noselect (expand-file-name "*.txt" qabf-dir)
                                 nil nil t)))
      (qa (bufferp b) t)
      (qa (buffer-name b) "*.txt")
      (kill-buffer b)))
  (delete-directory qabf-dir t))

;; == find-file-visit-truename / buffer-file-truename ==
(let* ((qabf-dir (make-temp-file "qabf-tn" t))
       (qabf-real (expand-file-name "real.txt" qabf-dir))
       (qabf-link (expand-file-name "link.txt" qabf-dir)))
  (write-region "R" nil qabf-real nil 'quiet)
  (make-symbolic-link qabf-real qabf-link)
  ;; default: buffer-file-name keeps the symlink path
  (let ((b (find-file-noselect qabf-link)))
    (qa (buffer-file-name b) qabf-link)
    ;; buffer-file-truename always resolves the link
    (qa (buffer-local-value 'buffer-file-truename b) (file-truename qabf-real))
    (kill-buffer b))
  ;; find-file-visit-truename is inert for find-file-noselect:
  ;; buffer-file-name still keeps the symlink path
  (let ((find-file-visit-truename t)
        (b (find-file-noselect qabf-link)))
    (qa (buffer-file-name b) qabf-link)
    (qa (buffer-local-value 'buffer-file-truename b) (file-truename qabf-real))
    (kill-buffer b))
  ;; but if the truename target is already visited, find-file-noselect
  ;; detects the alias and returns the SAME buffer
  (let ((br (find-file-noselect qabf-real)))
    (qa-same (find-file-noselect qabf-link) br)
    (kill-buffer br))
  (delete-directory qabf-dir t))

;; == modified flags / save-buffer / save-some-buffers ==
(let* ((qabf-dir (make-temp-file "qabf-save" t))
       (qabf-f (expand-file-name "s.txt" qabf-dir)))
  (write-region "orig\n" nil qabf-f nil 'quiet)
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      (qa (buffer-modified-p) nil)
      (insert "x")
      (qa (buffer-modified-p) t)
      (qa (buffer-modified-p b) t)
      (set-buffer-modified-p nil)
      (qa (buffer-modified-p) nil)
      ;; clearing the flag does not touch the recorded modtime
      (qa (verify-visited-file-modtime) t)
      ;; restore-buffer-modified-p nil clears the flag, returns nil
      (set-buffer-modified-p t)
      (qa (restore-buffer-modified-p nil) nil)
      (qa (buffer-modified-p) nil)
      ;; save-buffer writes and clears the flag
      (erase-buffer)
      (insert "new-content\n")
      (qa (save-buffer) nil)
      (qa (buffer-modified-p) nil)
      (qa (verify-visited-file-modtime) t))
    (kill-buffer b))
  (qa (with-temp-buffer (insert-file-contents qabf-f) (buffer-string))
      "new-content\n")
  ;; save-some-buffers t: t when a buffer was saved
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      (insert "more")
      (qa (save-some-buffers t) t)
      (qa (buffer-modified-p) nil))
    (kill-buffer b))
  ;; nil when nothing needed saving
  (qa (save-some-buffers t) nil)
  ;; save-buffer with no visited file: prompt hits EOF in batch
  (qa-err (with-temp-buffer (insert "x") (save-buffer)) 'end-of-file)
  (delete-directory qabf-dir t))

;; == modtime bookkeeping ==
(let* ((qabf-dir (make-temp-file "qabf-mt" t))
       (qabf-f (expand-file-name "m.txt" qabf-dir)))
  (write-region "M" nil qabf-f nil 'quiet)
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      (qa (listp (visited-file-modtime)) t)
      (qa (verify-visited-file-modtime) t)
      ;; integer modtime arg is rejected
      (qa-err (set-visited-file-modtime 12345) 'args-out-of-range)
      ;; a Lisp timestamp list works and mismatches the file
      (qa (set-visited-file-modtime '(0 0)) nil)
      (qa (visited-file-modtime) '(0 0 0 0))
      (qa (verify-visited-file-modtime) nil)
      ;; nil arg re-reads the file's modtime
      (qa (set-visited-file-modtime) nil)
      (qa (verify-visited-file-modtime) t)
      ;; clear: modtime becomes integer 0, verify still t
      (qa (clear-visited-file-modtime) nil)
      (qa (visited-file-modtime) 0)
      (qa (verify-visited-file-modtime) t))
    ;; buffer argument form
    (qa (verify-visited-file-modtime b) t)
    (kill-buffer b))
  ;; non-visiting buffer: verify is t, modtime is 0
  (qa (with-temp-buffer (verify-visited-file-modtime)) t)
  (qa (with-temp-buffer (visited-file-modtime)) 0)
  (delete-directory qabf-dir t))

;; == set-visited-file-name ==
(let* ((qabf-dir (make-temp-file "qabf-svn" t))
       (qabf-f (expand-file-name "v.txt" qabf-dir)))
  (write-region "V" nil qabf-f nil 'quiet)
  (let ((b (get-buffer-create "qasvn-buf")))
    (with-current-buffer b
      (insert "q")
      (set-buffer-modified-p t)
      ;; returns nil, sets buffer-file-name, renames the buffer,
      ;; keeps the modified flag, records the truename
      (qa (set-visited-file-name qabf-f) nil)
      (qa buffer-file-name qabf-f)
      (qa (buffer-name) "v.txt")
      (qa (buffer-modified-p) t)
      (qa buffer-file-truename (file-truename qabf-f)))
    (kill-buffer b))
  ;; nil clears the visited name
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      (set-visited-file-name nil)
      (qa buffer-file-name nil))
    (kill-buffer b))
  (delete-directory qabf-dir t))

;; == file locks ==
(let* ((qabf-dir (make-temp-file "qabf-lk" t))
       (qabf-f (expand-file-name "w.txt" qabf-dir))
       (qabf-lock (expand-file-name ".#w.txt" qabf-dir)))
  (write-region "orig\n" nil qabf-f nil 'quiet)
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      ;; no lock until the buffer is modified
      (qa (file-symlink-p qabf-lock) nil)
      (insert "y")
      ;; .#w.txt is a dangling symlink: file-symlink-p gives a string
      (qa (stringp (file-symlink-p qabf-lock)) t)
      (qa (file-locked-p qabf-f) t)
      ;; (lock-buffer) on our own lock: silent no-op
      (qa (lock-buffer) nil)
      (qa (unlock-buffer) nil)
      (qa (file-symlink-p qabf-lock) nil)
      (qa (file-locked-p qabf-f) nil)
      ;; re-lock, then release
      (qa (lock-buffer) nil)
      (qa (stringp (file-symlink-p qabf-lock)) t)
      (unlock-buffer)
      (set-buffer-modified-p nil))
    (kill-buffer b))
  ;; foreign lock: file-locked-p gives the login name only
  (make-symbolic-link "otheruser@otherhost.999:777" qabf-lock)
  (qa (file-locked-p qabf-f) "otheruser")
  ;; a lock by OUR user from a different pid still reports nil
  (delete-file qabf-lock)
  (make-symbolic-link (concat (user-login-name) "@" (system-name) ".1:1")
                      qabf-lock)
  (qa (file-locked-p qabf-f) nil)
  (delete-file qabf-lock)
  ;; foreign lock: first modify signals file-locked
  (make-symbolic-link "otheruser@otherhost.999:777" qabf-lock)
  (let ((b (find-file-noselect qabf-f)))
    (qa-err (with-current-buffer b (insert "z")) 'file-locked)
    (set-buffer-modified-p nil)
    (kill-buffer b))
  ;; ask-user-about-lock cannot ask in batch -> file-locked
  (qa-err (ask-user-about-lock qabf-f "otheruser@otherhost (pid 999)")
          'file-locked)
  (delete-file qabf-lock)
  (qa (file-locked-p "/no/such-qafile-xyz") nil)
  (delete-directory qabf-dir t))

;; == auto-save file names ==
(let* ((qabf-dir (make-temp-file "qabf-as" t))
       (qabf-f (expand-file-name "w.txt" qabf-dir)))
  (write-region "x" nil qabf-f nil 'quiet)
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      ;; variable, not function; nil until an auto-save happens
      (qa buffer-auto-save-file-name nil)
      (qa (make-auto-save-file-name)
          (expand-file-name "#w.txt#" qabf-dir))
      ;; auto-save-file-name-p returns an index, not t
      (qa (auto-save-file-name-p
           (file-name-nondirectory (make-auto-save-file-name))) 0)
      (qa (auto-save-file-name-p "#foo#") 0)
      (qa (auto-save-file-name-p buffer-file-name) nil)
      ;; a name with a slash is never an auto-save name
      (qa (auto-save-file-name-p "/a/#b#") nil)
      (qa (auto-save-file-name-p "foo") nil)
      ;; auto-save-mode returns the new state
      (qa (auto-save-mode 0) nil)
      (qa (auto-save-mode 1) t)
      (auto-save-mode 0))
    (kill-buffer b))
  ;; non-visiting buffer: generated name is still a string
  (qa (with-temp-buffer
        (stringp (make-auto-save-file-name)))
      t)
  ;; delete-auto-save-file-if-necessary keeps the file while modified
  (let* ((af (expand-file-name "as.txt" qabf-dir))
         (b (find-file-noselect af)))
    (with-current-buffer b
      (insert "aaa")
      (auto-save-mode 1)
      (let ((asf (expand-file-name "#as.txt#" qabf-dir)))
        (write-region "AUTO" nil asf nil 'quiet)
        (setq buffer-auto-save-file-name asf)
        (qa (let ((delete-auto-save-files t))
              (delete-auto-save-file-if-necessary))
            nil)
        (qa (file-exists-p asf) t)))
    (kill-buffer b))
  (delete-directory qabf-dir t))

;; == revert-buffer ==
(let* ((qabf-dir (make-temp-file "qabf-rv" t))
       (qabf-f (expand-file-name "r.txt" qabf-dir)))
  (write-region "REVERTED" nil qabf-f nil 'quiet)
  (let ((b (find-file-noselect qabf-f)))
    (with-current-buffer b
      (insert "zzzz")
      (qa (revert-buffer t t) t)
      (qa (buffer-string) "REVERTED")
      (qa (buffer-modified-p) nil))
    (kill-buffer b))
  ;; nothing to revert in a non-visiting buffer
  (qa-err (with-temp-buffer (insert "x") (revert-buffer t t)) 'error)
  (delete-directory qabf-dir t))

;; == buffer naming ==
(let ((b1 (get-buffer-create "qadup")))
  (qa-same (get-buffer-create "qadup") b1)
  ;; generate-new-buffer always makes a fresh, uniquified name
  (let ((b2 (generate-new-buffer "qadup")))
    (qa (buffer-name b2) "qadup<2>")
    (let ((b3 (generate-new-buffer "qadup")))
      (qa (buffer-name b3) "qadup<3>")
      (kill-buffer b3))
    (kill-buffer b2))
  (kill-buffer b1))
;; a killed buffer's name is immediately reusable
(let ((b (get-buffer-create "qareuse")))
  (kill-buffer b)
  (let ((b2 (generate-new-buffer "qareuse")))
    (qa (buffer-name b2) "qareuse")
    (kill-buffer b2)))
;; generate-new-buffer-name skips taken <N> forms
(let ((b1 (get-buffer-create "qagnb"))
      (b2 (get-buffer-create "qagnb<2>")))
  (qa (generate-new-buffer-name "qagnb") "qagnb<3>")
  (kill-buffer b1)
  (kill-buffer b2))
;; rename-buffer UNIQUE arg produces <2> on conflict
(let ((b1 (get-buffer-create "qarn-a"))
      (b2 (get-buffer-create "qarn-d")))
  (qa (with-current-buffer b2 (rename-buffer "qarn-a" t)) "qarn-a<2>")
  (qa (buffer-name b2) "qarn-a<2>")
  (kill-buffer b1)
  (kill-buffer b2))
;; rename-buffer without UNIQUE: plain error on conflict
(let ((b1 (get-buffer-create "qarn-c"))
      (b2 (get-buffer-create "qarn-e")))
  (qa-err (with-current-buffer b2 (rename-buffer "qarn-c")) 'error)
  (kill-buffer b1)
  (kill-buffer b2))
;; renaming to the current name is a no-op returning the name
(qa (with-temp-buffer (rename-buffer "qasame") (rename-buffer "qasame"))
       "qasame")
;; dead buffers: buffer-name nil, buffer-live-p nil
(let ((b (get-buffer-create "qadead")))
  (kill-buffer b)
  (qa (buffer-name b) nil)
  (qa (buffer-live-p b) nil))
;; buffer-file-name/truename nil in a non-visiting buffer
(qa (with-temp-buffer buffer-file-name) nil)
(qa (with-temp-buffer buffer-file-truename) nil)

(princ (qa--done))
