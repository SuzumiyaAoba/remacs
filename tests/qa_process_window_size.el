;;; -*- lexical-binding: t; -*-
;; Issue #247: validate uint16 dimensions and update the process input fd.
(defvar qa247-python
  (concat "import os,sys,signal\n"
          "signal.alarm(8)\n"
          "print('READY',flush=True)\n"
          "for line in sys.stdin:\n"
          " if line.strip()=='quit': break\n"
          " try:\n"
          "  s=os.get_terminal_size(1)\n"
          "  print('SIZE %d %d'%(s.columns,s.lines),flush=True)\n"
          " except OSError: print('NOTTY',flush=True)\n"))
(defun qa247-output (buffer)
  (with-current-buffer buffer (buffer-substring-no-properties (point-min) (point-max))))
(defun qa247-wait (process buffer pattern)
  (let ((deadline (+ (float-time) 5)))
    (while (and (not (string-match-p pattern (qa247-output buffer)))
                (process-live-p process) (< (float-time) deadline))
      (accept-process-output process 0.01))
    (unless (string-match-p pattern (qa247-output buffer)) (error "child response timeout"))))
(defun qa247-geometry (connection sizes)
  (let ((buffer (generate-new-buffer " *qa247-geometry*")) process)
    (unwind-protect
        (progn
          (setq process (make-process :name "qa247-geometry" :buffer buffer :noquery t
                         :sentinel #'ignore :connection-type connection :coding 'utf-8-unix
                         :command (list "/usr/bin/python3" "-u" "-c" qa247-python)))
          (qa247-wait process buffer "READY")
          (let (results)
            (dolist (size sizes)
              (with-current-buffer buffer (erase-buffer))
              (let ((result (set-process-window-size process (car size) (cadr size))))
                (process-send-string process "size\n")
                (qa247-wait process buffer "SIZE\\|NOTTY")
                (push (list result
                            (if (string-match "SIZE \\([0-9]+\\) \\([0-9]+\\)" (qa247-output buffer))
                                (list (string-to-number (match-string 1 (qa247-output buffer)))
                                      (string-to-number (match-string 2 (qa247-output buffer))))
                              'notty)) results)))
            (nreverse results)))
      (when process (ignore-errors (delete-process process)))
      (kill-buffer buffer))))
(defun qa247-arguments (height width &optional deleted)
  (let ((process (make-pipe-process :name "qa247-arguments" :noquery t :sentinel #'ignore)))
    (unwind-protect
        (progn (when deleted (delete-process process))
          (condition-case err (set-process-window-size process height width) (error err)))
      (ignore-errors (delete-process process)))))

(qa (qa247-geometry 'pty '((12 34))) '((t (34 12))))

(qa (qa247-geometry 'pipe '((12 34))) '((nil notty)))

(qa (qa247-geometry '(pty . pipe) '((12 34))) '((nil notty)))

(qa (qa247-geometry '(pipe . pty) '((12 34))) '((t (34 12))))

(qa (qa247-geometry '(pty . pty) '((12 34))) '((t (34 12))))

(qa (qa247-geometry 'pty '((0 0) (65535 65535) (24 80) (1 1)))
    '((t (0 0)) (t (65535 65535)) (t (80 24)) (t (1 1))))

(qa (qa247-arguments 0 0) 'nil)

(qa (qa247-arguments 65535 65535) 'nil)

(qa (qa247-arguments -1 1) '(args-out-of-range -1 0 65535))

(qa (qa247-arguments 1 -1) '(args-out-of-range -1 0 65535))

(qa (qa247-arguments 65536 1) '(args-out-of-range 65536 0 65535))

(qa (qa247-arguments 1 65536) '(args-out-of-range 65536 0 65535))

(qa (qa247-arguments 1.0 1) '(wrong-type-argument integerp 1.0))

(qa (qa247-arguments 1 2.0) '(wrong-type-argument integerp 2.0))

(qa (qa247-arguments nil 1) '(wrong-type-argument integerp nil))

(qa (qa247-arguments 1 t) '(wrong-type-argument integerp t))

(qa (qa247-arguments 'bad 'bad) '(wrong-type-argument integerp bad))

(qa (qa247-arguments (expt 2 200) 1)
    '(args-out-of-range
      1606938044258990275541962092341162602522202993782792835301376 0
      65535))

(qa (qa247-arguments 1 (- (expt 2 200)))
    '(args-out-of-range
      -1606938044258990275541962092341162602522202993782792835301376 0
      65535))

(qa (qa247-arguments 12 34 t) 'nil)

(qa (qa247-arguments 'bad 'bad t) '(wrong-type-argument integerp bad))

(qa
 (condition-case err
     (set-process-window-size 'not-a-process 'bad 'bad)
   (error err))
 '(wrong-type-argument processp not-a-process))

(qa
 (condition-case err (set-process-window-size "qa247-arguments" 12 34)
   (error err))
 '(wrong-type-argument processp "qa247-arguments"))

(qa
 (condition-case err (set-process-window-size nil 12 34) (error err))
 '(wrong-type-argument processp nil))

(qa
 (let
     ((process
       (make-network-process :name "qa247-server" :server t :host
			     "127.0.0.1" :service t :noquery t
			     :sentinel #'ignore)))
   (unwind-protect
       (list (set-process-window-size process 12 34)
	     (condition-case err
		 (set-process-window-size process -1 'bad)
	       (error err)))
     (delete-process process)))
 '(nil (args-out-of-range -1 0 65535)))


(princ (qa--done)) (terpri)
