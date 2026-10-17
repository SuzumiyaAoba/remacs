;;; -*- lexical-binding: nil -*-
;; Issue #245: a PTY write may consume only part of a large input buffer.
;; All children are ours, use no network, and self-terminate after 10 seconds.
;; GNU oracle: timeout 90 emacs -Q --batch -l tests/qa_framework.el \
;;                           -l tests/qa_process_large_send.el
(require 'timer)

(defvar qa245-line (concat (make-string 63 ?x) "\n"))
(defvar qa245-input (apply #'concat (make-list 2000 qa245-line)))
(defvar qa245-tail "TAIL:after-the-entire-outer-send\n")
(defvar qa245-output-line (concat (make-string 63 ?o) "\n"))
(defvar qa245-error-line (concat (make-string 63 ?e) "\n"))
(defvar qa245-python
  (concat
   "import os, sys, signal, time, termios, hashlib\n"
   "signal.alarm(10)\n"
   "mode = sys.argv[1]\n"
   "if os.isatty(0):\n"
   " a = termios.tcgetattr(0)\n"
   " a[3] = (a[3] | termios.ICANON) & ~(termios.ECHO | termios.ECHONL)\n"
   " a[1] &= ~termios.OPOST\n"
   " termios.tcsetattr(0, termios.TCSANOW, a)\n"
   "def put(fd, data):\n"
   " while data:\n"
   "  n = os.write(fd, data)\n"
   "  data = data[n:]\n"
   "put(1, b'READY\\n')\n"
   "if mode == 'blocked':\n"
   " time.sleep(9)\n"
   " sys.exit(0)\n"
   "if mode == 'interrupt': time.sleep(0.3)\n"
   "time.sleep(0.1)\n"
   "if mode == 'output':\n"
   " put(1, (b'o'*63+b'\\n')*2048)\n"
   "if mode == 'stderr':\n"
   " put(2, (b'e'*63+b'\\n')*2048)\n"
   " put(1, (b'o'*63+b'\\n')*2048)\n"
   "data = b''\n"
   "if mode == 'reenter':\n"
   " data = os.read(0, 64)\n"
   " put(1, b'REENTER\\n')\n"
   " time.sleep(0.1)\n"
   "while True:\n"
   " chunk = os.read(0, 8192)\n"
   " if not chunk: break\n"
   " data += chunk\n"
   "expected = (b'x'*63+b'\\n')*2000\n"
   "if mode == 'region':\n"
   " expected = ''.join('%06d:%056d\\n' % (n, n*17) for n in range(2000)).encode()\n"
   "if mode == 'reenter':\n"
   " expected += b'TAIL:after-the-entire-outer-send\\n'\n"
   "if data == expected:\n"
   " put(1, b'OK\\n')\n"
   "else:\n"
   " put(1, ('BAD %d %s\\n' % (len(data), hashlib.sha256(data).hexdigest())).encode())\n"))

(defun qa245-filter (process text)
  (with-current-buffer (process-buffer process)
    (goto-char (point-max))
    (insert text))
  (when (and (process-get process 'qa245-reenter)
             (not (process-get process 'qa245-triggered))
             (with-current-buffer (process-buffer process)
               (string-match-p "REENTER\n" (buffer-string))))
    (process-put process 'qa245-triggered t)
    (process-send-string process qa245-tail)
    (process-send-eof process)))

(defun qa245-wait (process ready)
  (let ((deadline (+ (float-time) 12)))
    (while (and (< (float-time) deadline)
                (if ready
                    (not (with-current-buffer (process-buffer process)
                           (string-match-p "READY\n" (buffer-string))))
                  (process-live-p process)))
      (accept-process-output nil 0.02))
    (when (if ready
              (not (with-current-buffer (process-buffer process)
                     (string-match-p "READY\n" (buffer-string))))
            (process-live-p process))
      (error "qa245 child did not reach %s" (if ready "ready" "exit")))
    ;; Drain residual output after the child status changes, including stderr.
    (unless ready (accept-process-output nil 0.05))))

(defun qa245-create (buffer mode &optional stderr)
  (let ((process
         (make-process :name "qa245-owned-child" :buffer buffer
                       :command (list "python3" "-u" "-c" qa245-python mode)
                       :connection-type 'pty :coding 'no-conversion
                       :filter #'qa245-filter :sentinel #'ignore
                       :stderr stderr :noquery t)))
    (condition-case err
        (progn (qa245-wait process t) process)
      (error (delete-process process) (signal (car err) (cdr err))))))

(defun qa245-transfer (mode)
  (let ((out (generate-new-buffer " *qa245-out*"))
        (err (and (equal mode "stderr")
                  (generate-new-buffer " *qa245-stderr*")))
        process)
    (unwind-protect
        (progn
          (setq process (qa245-create out mode err))
          (when err
            (let ((stderr-process (get-buffer-process err)))
              (when stderr-process
                (set-process-sentinel stderr-process #'ignore))))
          (when (equal mode "reenter")
            (process-put process 'qa245-reenter t))
          (let ((result
                 (if (equal mode "region")
                     (with-temp-buffer
                       ;; Delimiters outside the region must not be sent.
                       (insert "prefix-not-sent")
                       (let ((start (point)))
                         (dotimes (n 2000) (insert (format "%06d:%056d\n" n (* n 17))))
                         (let ((end (point)))
                           (insert "suffix-not-sent")
                           (process-send-region process start end))))
                   (process-send-string process qa245-input))))
            (unless (equal mode "reenter") (process-send-eof process))
            (qa245-wait process nil)
            (list result (process-exit-status process)
                  (with-current-buffer out (buffer-string))
                  (and err (with-current-buffer err (buffer-string)))
                  (and (process-get process 'qa245-triggered) t))))
      (when process (delete-process process))
      (when err
        (let ((stderr-process (get-buffer-process err)))
          (when stderr-process (delete-process stderr-process))))
      (kill-buffer out)
      (when err (kill-buffer err)))))

;; The issue's original 128000 bytes: 2000 short canonical-mode lines.
;; The child compares every byte, so a duplicated or reordered suffix fails too.
(qa (length qa245-input) 128000)
(qa (qa245-transfer "string") '(nil 0 "READY\nOK\n" nil nil))
;; Region boundaries and nonuniform contents exercise the other public sender.
(qa (qa245-transfer "region") '(nil 0 "READY\nOK\n" nil nil))
;; Output exceeds the PTY buffer before the child starts reading its stdin.
(qa (qa245-transfer "output")
    (list nil 0 (concat "READY\n" (apply #'concat (make-list 2048 qa245-output-line)) "OK\n") nil nil))
;; A separately captured stderr pipe must be pumped as well as the main PTY.
(qa (qa245-transfer "stderr")
    (list nil 0 (concat "READY\n" (apply #'concat (make-list 2048 qa245-output-line)) "OK\n")
          (apply #'concat (make-list 2048 qa245-error-line)) nil))
;; The filter reenters process-send-string while the outer send is blocked;
;; its tail and EOF must follow all bytes of that outer call.
(qa (qa245-transfer "reenter") '(nil 0 "READY\nREENTER\nOK\n" nil t))

(defun qa245-delete-during-send ()
  (let ((out (generate-new-buffer " *qa245-delete*")) process timer fired result)
    (unwind-protect
        (progn
          (setq process (qa245-create out "blocked"))
          (setq timer (run-at-time 0.05 nil
                                   (lambda () (setq fired t) (delete-process process))))
          (setq result (condition-case nil
                           (progn (process-send-string process qa245-input) 'returned)
                         (error 'error)))
          (list fired result (and (process-live-p process) t)))
      (when timer (cancel-timer timer))
      (when process (delete-process process))
      (kill-buffer out))))
;; No outstanding RefCell borrow may survive the callback deleting its child.
(qa (qa245-delete-during-send) '(t error nil))

(defun qa245-throw-during-send ()
  (let ((out (generate-new-buffer " *qa245-throw*")) process timer result)
    (unwind-protect
        (progn
          (setq process (qa245-create out "interrupt"))
          (setq timer (run-at-time 0.05 nil (lambda () (throw 'qa245-stop 'interrupted))))
          (setq result (catch 'qa245-stop
                         (process-send-string process qa245-input)
                         'returned))
          ;; An interrupted send retains its suffix.  Even an empty later
          ;; send must drain that suffix before EOF can reach the reader.
          (process-send-string process "")
          (process-send-eof process)
          (qa245-wait process nil)
          (list result (process-exit-status process)
                (with-current-buffer out (buffer-string))))
      (when timer (cancel-timer timer))
      (when process (delete-process process))
      (kill-buffer out))))
;; A nonlocal exit must unwind safely and preserve the queued input for retry.
(qa (qa245-throw-during-send) '(interrupted 0 "READY\nOK\n"))

(princ (qa--done)) (terpri)
