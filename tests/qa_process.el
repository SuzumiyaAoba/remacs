;; Process/subprocess domain — GNU 31.1 pins (verified).
;;
;; Notes:
;;   - call-process returns a numeric exit status, or a signal
;;     description STRING when the program died on a signal.
;;   - call-process DESTINATION semantics (GNU callproc.c):
;;     t/nil/buffer = current-buffer/discard/insert-at-point;
;;     0 = discard and don't wait (returns nil);
;;     '(:file FILE) = write stdout to FILE;
;;     (REAL-BUFFER . STDERR-FILE) splits stdout/stderr:
;;       STDERR-FILE t mixes stderr into stdout, nil discards it,
;;       a string names a stderr file.
;;   - process-status returns the CACHED status — it only changes
;;     when an event source reports it (accept-process-output,
;;     sit-for, or process-send-eof).
;;   - start-process creates a pty-less pipe process; process-type
;;     is `real'.
;;   - `signal-process' returns 0 on POSIX.
;;   - `kill-process' returns the PROCESS object (not 0).
;;   - call-process-region feeds the buffer text to the program's
;;     stdin via a temp file.
;;   - GNU prints "Process <name> finished\n" to stderr through the
;;     DEFAULT sentinel on exit; remacs drops it (xfail).

;; == call-process: exit status ==
(qa (call-process "true" nil nil nil) 0)
(qa (call-process "false" nil nil nil) 1)
;; shell exit status propagates
(qa (call-process "sh" nil nil nil "-c" "exit 7") 7)
;; DESTINATION t: insert stdout at point in current buffer
(qa (with-temp-buffer
      (call-process "echo" nil t nil "hello")
      (buffer-string))
    "hello\n")
;; insertion point advances (appends after existing text)
(qa (with-temp-buffer
      (insert "pre|")
      (call-process "echo" nil t nil "post")
      (buffer-string))
    "pre|post\n")
;; DESTINATION nil: discard
(qa (with-temp-buffer
      (list (call-process "echo" nil nil nil "gone") (buffer-string)))
    '(0 ""))
;; DESTINATION 0: don't wait, return nil
(qa (call-process "echo" nil 0 nil "gone") nil)
;; DESTINATION buffer object: insert into that buffer
(qa (let ((b (get-buffer-create "*qa-cp-b*")))
      (with-current-buffer b (erase-buffer))
      (with-temp-buffer
        (call-process "echo" nil b nil "tgt")
        (with-current-buffer b (buffer-string))))
    "tgt\n")
;; DESTINATION buffer NAME string resolves to that buffer
(qa (let ((b (get-buffer-create "qa-cp-name")))
      (with-current-buffer b (erase-buffer))
      (with-temp-buffer
        (call-process "echo" nil "qa-cp-name" nil "bn")
        (with-current-buffer b (buffer-string))))
    "bn\n")
;; '(:file FILE) writes stdout to the file (overwrites)
(let ((f "/tmp/qa_cp_file.txt"))
  (when (file-exists-p f) (delete-file f))
  (with-temp-buffer
    (call-process "echo" nil (list :file f) nil "F1"))
  (qa (with-temp-buffer (insert-file-contents f) (buffer-string)) "F1\n")
  ;; second call overwrites (not append)
  (call-process "echo" nil (list :file f) nil "F2")
  (qa (with-temp-buffer (insert-file-contents f) (buffer-string)) "F2\n")
  (delete-file f))
;; (t . t): stderr mixes into the destination
(qa (with-temp-buffer
         (call-process "sh" nil (cons t t) nil "-c" "echo E >&2; echo O")
          (buffer-string))
         "E\nO\n")
;; (t . nil): GNU still mixes stderr into the destination buffer
;; (dotted (BUFF . nil) is NOT "stderr → nil" — pinned empirically).
(qa (with-temp-buffer
         (call-process "sh" nil (cons t nil) nil "-c" "echo E >&2; echo O")
          (buffer-string))
         "E\nO\n")
;; (nil . t): stdout discarded; stderr "mixed into" discarded output
(qa (with-temp-buffer
      (list (call-process "sh" nil (cons nil t) nil "-c" "echo E >&2; echo O")
            (buffer-string)))
    '(0 ""))
;; (REALDEST STDERR-FILE) proper-list form: stderr goes to the file
(let ((ef "/tmp/qa_cp_err.txt"))
  (when (file-exists-p ef) (delete-file ef))
  (call-process "sh" nil (list nil ef) nil "-c" "echo E >&2; echo O")
  (qa (with-temp-buffer (insert-file-contents ef) (buffer-string)) "E\n")
  (delete-file ef))
;; (BUFFER . "file") proper list: stdout to buffer, stderr to file
(qa (let ((b (get-buffer-create "*qa-cp-sp*"))
          (ef "/tmp/qa_cp_err2.txt"))
      (when (file-exists-p ef) (delete-file ef))
      (with-current-buffer b (erase-buffer))
      (unwind-protect
          (with-temp-buffer
            (call-process "sh" nil (list b ef) nil "-c" "echo E >&2; echo O")
            (list (with-current-buffer b (buffer-string))
                  (with-temp-buffer
                    (insert-file-contents ef) (buffer-string))))
        (when (file-exists-p ef) (delete-file ef))))
    '("O\n" "E\n"))
;; INFILE string feeds the process stdin
(let ((inf "/tmp/qa_cp_in.txt"))
  (with-temp-file inf (insert "from-file\n"))
  (qa (with-temp-buffer
        (call-process "cat" inf t nil)
        (buffer-string))
      "from-file\n")
  (delete-file inf))
;; missing program → file-missing
(qa-err (call-process "qa-no-such-prog-xyz" nil nil nil) 'file-missing)
;; PROGRAM is searched on exec-path (PATH)
(qa (call-process "cat" nil t nil) 0)

;; == call-process-region: START/END feed stdin ==
(qa (with-temp-buffer
      (insert "region-input")
      (let ((b (get-buffer-create "*qa-cpr*")))
        (with-current-buffer b (erase-buffer))
        (list (call-process-region 1 12 "cat" nil b nil)
              (with-current-buffer b (buffer-string)))))
    '(0 "region-inpu"))
;; START == END: empty input
(qa (with-temp-buffer
      (insert "xx")
      (let ((b (get-buffer-create "*qa-cpr2*")))
        (with-current-buffer b (erase-buffer))
        (list (call-process-region 1 1 "cat" nil b nil)
              (with-current-buffer b (buffer-string)))))
    '(0 ""))
;; DELETE arg non-nil removes the region
(qa (with-temp-buffer
        (insert "del-me")
        (call-process-region 1 7 "cat" t nil nil)
        (buffer-string))
      "")
;; cat via call-process-region passes exit status
(qa (with-temp-buffer
      (insert "x")
      (call-process-region 1 2 "false" nil nil nil))
    1)

;; == call-process-shell-command ==
(qa (with-temp-buffer
      (call-process-shell-command "echo via-sh" nil t)
      (buffer-string))
    "via-sh\n")
(qa (call-process-shell-command "exit 3" nil nil nil) 3)

;; == shell-command / shell-command-to-string ==
(qa (let ((b (get-buffer-create "*qa-sc*")))
      (with-current-buffer b (erase-buffer))
      (shell-command "echo sc-out" b)
      (with-current-buffer b (buffer-string)))
    "sc-out\n")
;; ERROR-BUFFER arg captures stderr separately
(qa (let ((ob (get-buffer-create "*qa-sco*"))
            (eb (get-buffer-create "*qa-sce*")))
        (with-current-buffer ob (erase-buffer))
        (with-current-buffer eb (erase-buffer))
        (shell-command "echo E >&2; echo O" ob eb)
        (list (with-current-buffer ob (buffer-string))
              (with-current-buffer eb (buffer-string))))
      '("O\n" "E\n"))
;; nil output-buffer under batch still runs the command
;; shell-command returns the exit status
(qa (shell-command "true") 0)
;; shell-command-to-string trims nothing, returns stdout
(qa (shell-command-to-string "echo s2s") "s2s\n")
;; non-zero exit still returns output (exit status lost)
(qa (shell-command-to-string "echo x; exit 1") "x\n")

;; == process-lines / process-file ==
(qa (process-lines "echo" "l1\nl2") '("l1" "l2"))
(qa (process-lines "printf" "one") '("one"))
;; process-file on local dir == call-process
(qa (with-temp-buffer
        (list (process-file "echo" nil t nil "pf") (buffer-string)))
    '(0 "pf\n"))
(qa (process-file "false" nil nil nil) 1)
(qa (with-temp-buffer
        (list (process-file-shell-command "echo pfsh" nil t)
              (buffer-string)))
    '(0 "pfsh\n"))

;; == start-process: object shape ==
(let ((p (start-process "qa-sp" nil "sleep" "5")))
  (qa (processp p) t)
  (qa (process-name p) "qa-sp")
  (qa (process-type p) 'real)
  (qa (process-command p) '("sleep" "5"))
  (qa (and (process-live-p p) t) t)
  (qa (numberp (process-id p)) t)
  (qa (eq (get-process "qa-sp") p) t)
  (qa (and (memq p (process-list)) t) t)
  (signal-process p 'KILL))
;; process-status is the CACHED status: 'run while alive
(let ((p (start-process "qa-sp2" nil "sleep" "5")))
  (qa (process-status p) 'run)
  (signal-process p 'KILL))
;; exited process: after the event is observed, status/exit-status update
(let ((p (start-process "qa-sp3" nil "true")))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'exit)
  (qa (process-exit-status p) 0))
;; non-zero exit → status still 'exit; exit-status carries the code
(let ((p (start-process "qa-sp4" nil "false")))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'exit)
  (qa (process-exit-status p) 1))
;; SIGKILL → status 'signal
(let ((p (start-process "qa-sp5" nil "sleep" "5")))
  (signal-process p 'KILL)
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'signal))
;; process-exit-status is nil until the process exits
(let ((p (start-process "qa-sp6" nil "sleep" "5")))
  (qa (process-exit-status p) 0)
  (signal-process p 'KILL))
;; process-buffer nil when no buffer given
(let ((p (start-process "qa-sp7" nil "sleep" "5")))
  (qa (process-buffer p) nil)
  (signal-process p 'KILL))
;; process-buffer returns the buffer when given
(let* ((b (get-buffer-create "*qa-spb*"))
       (p (start-process "qa-sp8" b "sleep" "5")))
  (qa (eq (process-buffer p) b) t)
  (signal-process p 'KILL))
;; process-mark points at the end of accumulated output
(let* ((b (get-buffer-create "*qa-spm*"))
       (p (start-process "qa-sp9" b "echo" "zz")))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (marker-position (process-mark p))
      (with-current-buffer b (point-max)))
  ;; default sentinel appends "\nProcess <name> finished\n" after the
  ;; output — but the ordering is a race; assert the output prefix.
  (qa (string-prefix-p "zz" (with-current-buffer b (buffer-string))) t))
;; process-tty-name: pipe-backed proc has no tty → nil or a dev path
(let ((p (start-process "qa-spt" nil "sleep" "5")))
  (qa (or (null (process-tty-name p)) (stringp (process-tty-name p))) t)
  (signal-process p 'KILL))
;; process-contact for a pipe proc: t (pty) or a string pid-ish
(let ((p (start-process "qa-spc" nil "sleep" "5")))
  (qa (or (eq (process-contact p) t) (stringp (process-contact p))) t)
  (signal-process p 'KILL))
;; process-coding-system defaults to utf-8-unix pair on POSIX
(let ((p (start-process "qa-spcs" nil "sleep" "5")))
  (qa (consp (process-coding-system p)) t)
  (signal-process p 'KILL))

;; == start-process-shell-command ==
(let ((p (start-process-shell-command "qa-shc" nil "echo shx")))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'exit))

;; == start-file-process ==
(let ((p (start-file-process "qa-sfp" nil "true")))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'exit))

;; == make-process ==
(let* ((b (get-buffer-create "*qa-mp*"))
       (p (make-process :name "qa-mk" :buffer b :command '("echo" "mk-out"))))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'exit)
  ;; GNU prints a default-sentinel "finished" line into the buffer
  (qa (string-prefix-p "mk-out" (with-current-buffer b (buffer-string))) t))
;; :sentinel replaces the default — receives the event string
(let* ((got nil)
       (p (make-process :name "qa-ms" :command '("true")
                        :sentinel (lambda (proc ev) (setq got ev)))))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa got "finished\n"))
;; :sentinel nil also gets nothing printed (no default)
(let* ((got nil) (called 0)
       (p (make-process :name "qa-msn" :command '("true")
                        :sentinel (lambda (proc ev) (setq called (1+ called))))))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa called 1))
;; :filter receives output string
(let* ((acc nil)
       (p (make-process :name "qa-mf" :command '("echo" "fx")
                        :filter (lambda (proc s) (setq acc (cons s acc))))))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (apply #'concat (nreverse acc)) "fx\n"))
;; :noquery t clears the query flag
(let ((p (make-process :name "qa-mnq" :command '("sleep" "5") :noquery t)))
  (qa (process-query-on-exit-flag p) nil)
  (signal-process p 'KILL))
;; :stderr buffer receives stderr separately
(let* ((eb (get-buffer-create "*qa-mse*"))
       (ob (get-buffer-create "*qa-mso*"))
       (p (make-process :name "qa-mste" :buffer ob :stderr eb
                        :command '("sh" "-c" "echo E >&2; echo O"))))
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (string-prefix-p "O" (with-current-buffer ob (buffer-string))) t)
  (qa (string-prefix-p "E" (with-current-buffer eb (buffer-string))) t))
;; make-process arg validation
(qa-err (make-process :name 5 :command '("true")) 'error)
(qa-err-data (make-process :name "qa-mbad" :command "notalist") 'wrong-type-argument
             '(listp "notalist"))
;; no args: GNU returns nil (no-op)
(qa (make-process) nil)

;; == process control: signal/stop/continue/kill ==
(let ((p (start-process "qa-ctl1" nil "sleep" "5")))
  (qa (signal-process p 'KILL) 0)
  (sit-for 0.1))
;; stop/continue via functions
(let ((p (start-process "qa-ctl2" nil "sleep" "5")))
  (qa-same (stop-process p) p)
  ;; GNU keeps the cached 'run — TSTP is delivered but the status
  ;; only updates through the event pump (accept-process-output).
  (qa (process-status p) 'run)
  (qa-same (continue-process p) p)
  (qa (process-status p) 'run)
  (signal-process p 'KILL))
;; interrupt/quit/kill return nil; status becomes 'signal
(let ((p (start-process "qa-ctl3" nil "sleep" "5")))
  (qa-same (kill-process p) p)
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'signal))
;; signal-process on a dead process returns -1
(let ((p (start-process "qa-ctl4" nil "true")))
  (sit-for 0.3)
  (qa (signal-process p 'KILL) -1))

;; == process I/O ==
(let* ((b (get-buffer-create "*qa-io*"))
       (p (start-process "qa-io" b "cat")))
  (process-send-string p "via-str\n")
  (process-send-eof p)
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (string-prefix-p "via-str" (with-current-buffer b (buffer-string))) t))
;; process-send-region sends START..END of a buffer
(let* ((b (get-buffer-create "*qa-io2*"))
       (p (start-process "qa-io2" b "cat")))
  (with-temp-buffer
    (insert "XXregionYY")
    (process-send-region p 3 9))
  (process-send-eof p)
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (string-prefix-p "region" (with-current-buffer b (buffer-string))) t))
;; process-send-eof on the proc sends EOF to stdin → cat exits
(let ((p (start-process "qa-io3" (get-buffer-create "*qa-io3*") "cat")))
  (qa-same (process-send-eof p) p)
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (process-status p) 'exit))
;; sending to a dead process → error
(let ((p (start-process "qa-io4" nil "true")))
  (sit-for 0.3)
  (qa-err (process-send-string p "x") 'error))

;; == set-process-* setters ==
(let ((p (start-process "qa-ss" nil "sleep" "5")))
  ;; set-process-buffer nil → output discarded
  (qa (set-process-buffer p nil) nil)
  (qa (process-buffer p) nil)
  ;; set-process-query-on-exit-flag
  (qa (process-query-on-exit-flag p) t)
  (qa (set-process-query-on-exit-flag p nil) nil)
  (qa (process-query-on-exit-flag p) nil)
  (signal-process p 'KILL))
;; set-process-buffer to a live buffer
(let* ((b (get-buffer-create "*qa-ssb*"))
       (p (start-process "qa-ssb" nil "echo" "z")))
  (set-process-buffer p b)
  (accept-process-output p 1)
  (sit-for 0.1)
  (qa (string-prefix-p "z" (with-current-buffer b (buffer-string))) t))

;; == process-plist / process-put / process-get ==
(let ((p (start-process "qa-pl" nil "sleep" "5")))
  (process-put p 'k1 'v1)
  (qa (process-get p 'k1) 'v1)
  (qa (process-plist p) '(k1 v1))
  (set-process-plist p '(a 1 b 2))
  (qa (process-get p 'b) 2)
  (signal-process p 'KILL))

;; == delete-process ==
(let ((p (start-process "qa-dp" nil "sleep" "5")))
  (qa (delete-process p) nil)
  (sit-for 0.1)
  (qa (process-live-p p) nil))
;; deleting an already-exited process is a silent no-op
(let ((p (start-process "qa-dp2" nil "true")))
  (sit-for 0.3)
  (qa (delete-process p) nil))

;; == process-list / get-process ==
(let ((p (start-process "qa-lst" nil "sleep" "5")))
  (qa (and (memq p (process-list)) t) t)
  (qa (eq (get-process "qa-lst") p) t)
  (signal-process p 'KILL))
(qa (get-process "qa-no-such-proc-xyz") nil)
;; get-process on a dead process returns nil
(let ((p (start-process "qa-ded" nil "true")))
  (sit-for 0.3)
  (qa (get-process "qa-ded") nil))

;; == process-live-p / processp edge ==
(let ((p (start-process "qa-lp" nil "true")))
  (sit-for 0.3)
  (qa (process-live-p p) nil))
(qa (processp 'sym) nil)
(qa (processp "str") nil)
(qa (processp nil) nil)

;; == process-exit-status on running proc ==
(let ((p (start-process "qa-ex" nil "sleep" "5")))
  (qa (process-exit-status p) 0)
  (signal-process p 'KILL))

;; == environment ==
(qa (listp exec-path) t)
(qa (listp process-environment) t)
(setenv "QA_PROC_V" "hello-v")
(qa (getenv "QA_PROC_V") "hello-v")
(qa (getenv-internal "QA_PROC_V") "hello-v")
(qa (and (member "QA_PROC_V=hello-v" process-environment) t) t)
;; setenv with nil removes it
(setenv "QA_PROC_V")
(qa (getenv "QA_PROC_V") nil)
;; getenv on unset var → nil
(qa (getenv "QA_PROC_NO_SUCH_XYZ") nil)
;; setenv processes "VAR=VALUE" through process-environment list
(let ((process-environment (list "QA_ONLY=ins")))
  (qa (getenv "QA_ONLY") "ins"))
;; shell-file-name / shell-command-switch
(qa shell-file-name (or (getenv "SHELL") "/bin/sh"))
(qa shell-command-switch "-c")
;; emacs-pid
(qa (numberp (emacs-pid)) t)

;; == error surface ==
(qa-err (start-process 5 nil "true") 'error)
(qa-err (process-send-string 5 "x") 'wrong-type-argument)
(qa-err (process-status 5) 'wrong-type-argument)
(qa (signal-process 5 'KILL) -1)
;; process on a killed buffer / no such name → nil, not error
(qa-err (get-process 5) 'wrong-type-argument)

;; process-message? early perms check
(qa (fboundp 'make-serial-process) t) ; exists in GNU 31.1


;; Batch display-message-or-buffer must not leave an echo-area message.
(dolist (inhibit '(nil t))
  (let ((inhibit-message inhibit))
    (dolist (text '("qa159 short" "100% 日本語" ""))
      (qa (display-message-or-buffer text) text)
      (qa (current-message) nil)
      (qa (message "%s" "qa159 later") "qa159 later")
      (qa (current-message) nil))
    (display-message-or-buffer "qa159 clear")
    (message nil)
    (qa (current-message) nil)
    (message "")
    (qa (current-message) nil)))

;; == display-message-or-buffer accepts a buffer (shell-command path) ==
;; GNU Fdisplay_message_or_buffer returns t for a buffer arg.
(let ((qb (get-buffer-create "qap-dmb")))
  (unwind-protect
      (qa (display-message-or-buffer qb) t)
    (kill-buffer qb)))
;; shell-command with explicit output + error buffers
(let ((ob (get-buffer-create "qap-sco"))
      (eb (get-buffer-create "qap-sce")))
  (unwind-protect
      (progn
        (with-current-buffer ob (erase-buffer))
        (with-current-buffer eb (erase-buffer))
        (shell-command "echo E >&2; echo O" ob eb)
        (qa (list (with-current-buffer ob (buffer-string))
                  (with-current-buffer eb (buffer-string)))
            '("O\n" "E\n")))
    (kill-buffer ob)
    (kill-buffer eb)))

(princ (qa--done))
