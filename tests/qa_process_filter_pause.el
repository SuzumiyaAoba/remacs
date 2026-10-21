;;; -*- lexical-binding: t; -*-
(defvar qa206-mode)

(defun qa206-until (process predicate)
  (let ((deadline (+ (float-time) 3)))
    (while (and (not (funcall predicate)) (< (float-time) deadline))
      (accept-process-output process 0.01)))
  (funcall predicate))

(defun qa206-live (mode set-later send-paused)
  (let ((buffer (generate-new-buffer " *qa206-live*")) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "qa206-live" :buffer buffer :noquery t
                                :connection-type mode :sentinel #'ignore
                                :filter (unless set-later t)
                                :command '("/bin/sh" "-c" "printf OLD; read one; printf NEW; read two")))
          (when set-later (set-process-filter p t))
          (when send-paused (process-send-string p "go\n"))
          (let ((filter (eq (process-filter p) t))
                (quiet (null (accept-process-output p 0.03 nil t)))
                (empty (with-current-buffer buffer (= (buffer-size) 0)))
                (live (not (null (process-live-p p)))))
            (set-process-filter p nil)
            (unless send-paused (process-send-string p "go\n"))
            (qa206-until p (lambda () (with-current-buffer buffer (= (buffer-size) 6))))
            (list filter quiet empty live (with-current-buffer buffer (buffer-string)))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

(defun qa206-bulk (mode reentrant)
  (let ((text "") (calls 0) (nested 'not-called) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "qa206-bulk" :noquery t :sentinel #'ignore
                                :connection-type mode :coding 'utf-8-unix
                                :filter (lambda (process string)
                                          (setq calls (1+ calls) text (concat text string))
                                          (set-process-filter process t)
                                          (when reentrant
                                            (setq nested (accept-process-output process 0.01 nil t))))
                                :command '("/bin/sh" "-c" "printf '%050000d' 0; read done")))
          (qa206-until p (lambda () (> calls 0)))
          (let ((quiet (null (accept-process-output p 0.03 nil t))))
            (set-process-filter p (lambda (_process string) (setq text (concat text string))))
            (process-send-string p "go\n")
            (qa206-until p (lambda () (= (length text) 50000)))
            (list calls quiet (if reentrant (null nested) t)
                  (equal text (make-string 50000 ?0)))))
      (when (and p (process-live-p p)) (delete-process p)))))

(defun qa206-exited (mode)
  (let ((buffer (generate-new-buffer " *qa206-exit*")) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "qa206-exit" :buffer buffer :noquery t
                                :filter t :sentinel #'ignore :connection-type mode
                                :command '("/bin/sh" "-c" "printf discarded")))
          (if (eq mode 'pty)
              ;; The full PTY's writer waits for output to be read before
              ;; exiting; a pipe child can exit with unread output queued.
              (accept-process-output nil 0.03)
            (qa206-until nil (lambda () (not (process-live-p p)))))
          (let ((before (process-status p)))
            (set-process-filter p nil)
            (qa206-until nil (lambda () (not (process-live-p p))))
            (list before (process-status p) (with-current-buffer buffer (buffer-string)))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

(dolist (qa206-mode '(pipe pty))
  (qa (qa206-live qa206-mode nil nil) '(t t t t "OLDNEW"))
  (qa (qa206-live qa206-mode t nil) '(t t t t "OLDNEW"))
  (qa (qa206-live qa206-mode nil t) '(t t t t "OLDNEW"))
  (qa (qa206-bulk qa206-mode nil) '(1 t t t))
  (qa (qa206-exited qa206-mode)
      (if (eq qa206-mode 'pty) '(run exit "discarded") '(exit exit ""))))
(qa (qa206-exited '(pipe . pty)) '(exit exit ""))
(qa (qa206-exited '(pty . pipe)) '(exit exit ""))
(qa (qa206-bulk 'pipe t) '(1 t t t))

;; Pending decoder bytes cannot reappear after a paused child exits.
(qa-lex
 (let ((buffer (generate-new-buffer " *qa206-partial*")) (first nil) p)
   (unwind-protect
       (progn
         (setq p (make-process :name "qa206-partial" :buffer buffer :noquery t
                              :coding 'utf-8-unix :connection-type 'pipe :sentinel #'ignore
                              :command '("/bin/sh" "-c" "printf '\\343'; read go; printf '\\201\\202'")))
         (let ((deadline (+ (float-time) 3)))
           (while (and (not first) (< (float-time) deadline))
             (setq first (accept-process-output p 0.01 nil t))))
         (set-process-filter p t)
         (process-send-string p "go\n")
         (qa206-until nil (lambda () (not (process-live-p p))))
         (set-process-filter p nil)
         (accept-process-output p 0.01 nil t)
         (list first (with-current-buffer buffer (buffer-string))))
     (when (and p (process-live-p p)) (delete-process p))
     (kill-buffer buffer)))
 '(t ""))

;; Separate stderr continues to arrive while stdout is paused, and
;; satisfies a global wait but not a wait for the child's stdout.
(defun qa206-stderr (global)
  (let ((output (generate-new-buffer " *qa206-out*"))
        (errors (generate-new-buffer " *qa206-err*")) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "qa206-stderr" :buffer output :stderr errors
                                :filter t :noquery t :sentinel #'ignore :connection-type 'pipe
                                :command '("/bin/sh" "-c" "printf OUT; printf ERR >&2; read done")))
          (let ((got (accept-process-output (unless global p) 0.1)))
            (let ((before (list (not (null got))
                                (with-current-buffer output (buffer-string))
                                (with-current-buffer errors (buffer-string)))))
              (set-process-filter p nil)
              (qa206-until p (lambda () (with-current-buffer output (= (buffer-size) 3))))
              (append before (list (with-current-buffer output (buffer-string)))))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer output) (kill-buffer errors))))
(qa (qa206-stderr nil) '(nil "" "ERR" "OUT"))
(qa (qa206-stderr t) '(t "" "ERR" "OUT"))

;; Only the canonical t symbol stops input.
(qa-lex
 (let ((filter (make-symbol "t")) (text "") p)
   (unwind-protect
       (progn
         (fset filter (lambda (_process string) (setq text (concat text string))))
         (setq p (make-process :name "qa206-symbol" :filter filter :noquery t
                              :sentinel #'ignore :connection-type 'pipe
                              :command '("/bin/sh" "-c" "printf X; read done")))
         (qa206-until p (lambda () (equal text "X")))
         (list (eq (process-filter p) filter) text))
     (when (and p (process-live-p p)) (delete-process p))))
 '(t "X"))

(defun qa206-pipe (close-child)
  (let ((buffer (generate-new-buffer " *qa206-pipe*"))
        (ready (generate-new-buffer " *qa206-ready*")) pipe child)
    (unwind-protect
        (progn
          (setq pipe (make-pipe-process :name "qa206-pipe" :buffer buffer :filter t
                                        :noquery t :sentinel #'ignore)
                child (make-process :name "qa206-writer" :buffer ready :stderr pipe
                                    :noquery t :sentinel #'ignore :connection-type 'pipe
                                    :command '("/bin/sh" "-c" "printf PIPE >&2; printf READY; read done")))
          (qa206-until child (lambda () (with-current-buffer ready (= (buffer-size) 5))))
          (when close-child (delete-process child))
          (let ((quiet (null (accept-process-output pipe 0.03 nil t)))
                (empty (with-current-buffer buffer (= (buffer-size) 0)))
                (live (not (null (process-live-p pipe)))))
            (set-process-filter pipe nil)
            (qa206-until pipe (lambda () (with-current-buffer buffer (= (buffer-size) 4))))
            (when close-child
              (qa206-until pipe (lambda () (not (process-live-p pipe)))))
            (list quiet empty live (with-current-buffer buffer (buffer-string))
                  (if close-child (not (process-live-p pipe)) t))))
      (when (and child (process-live-p child)) (delete-process child))
      (when (and pipe (process-live-p pipe)) (delete-process pipe))
      (kill-buffer buffer) (kill-buffer ready))))
(qa (qa206-pipe nil) '(t t t "PIPE" t))
(qa (qa206-pipe t) '(t t t "PIPE" t))

(defun qa206-network (local bulk)
  (let* ((socket (and local (make-temp-file "qa206-socket-")))
         (client-buffer (generate-new-buffer " *qa206-client*"))
         (peer-buffer (generate-new-buffer " *qa206-peer*"))
         (text "") (calls 0) server client peer)
    (unwind-protect
        (progn
          (when socket (delete-file socket))
          (setq server (apply #'make-network-process :name "qa206-server" :server t
                              :filter t :noquery t :sentinel #'ignore
                              (if local (list :family 'local :service socket)
                                '(:family ipv4 :host "127.0.0.1" :service t))))
          (let ((before (process-list)))
            (setq client (apply #'make-network-process :name "qa206-client" :buffer client-buffer
                                :filter t :noquery t :sentinel #'ignore
                                (if local (list :family 'local :service socket)
                                  (list :family 'ipv4 :host "127.0.0.1"
                                        :service (process-contact server :service)))))
            (qa206-until nil
                         (lambda ()
                           (dolist (candidate (process-list))
                             (unless (or (eq candidate client) (memq candidate before))
                               (setq peer candidate)))
                           peer)))
          (unless peer (error "No accepted connection"))
          (set-process-buffer peer peer-buffer)
          (if bulk
              (progn
                (set-process-filter client (lambda (process string)
                                             (setq calls (1+ calls) text (concat text string))
                                             (set-process-filter process t)))
                (process-send-string peer (make-string 4096 ?x))
                (qa206-until client (lambda () (> calls 0)))
                (process-send-string peer "TAIL")
                (let ((quiet (null (accept-process-output client 0.03 nil t))))
                  (set-process-filter client (lambda (_process string) (setq text (concat text string))))
                  (qa206-until client (lambda () (= (length text) 4100)))
                  (list calls quiet (equal text (concat (make-string 4096 ?x) "TAIL")))))
            (let ((inherited (and (eq (process-filter server) t) (eq (process-filter peer) t))))
              (process-send-string peer "DATA")
              (let ((quiet (null (accept-process-output client 0.03 nil t)))
                    (empty (with-current-buffer client-buffer (= (buffer-size) 0))))
                (set-process-filter client nil)
                (qa206-until client (lambda () (with-current-buffer client-buffer (= (buffer-size) 4))))
                (process-send-string client "PEER")
                (let ((peer-quiet (null (accept-process-output peer 0.03 nil t)))
                      (peer-empty (with-current-buffer peer-buffer (= (buffer-size) 0))))
                  (set-process-filter peer nil)
                  (qa206-until peer (lambda () (with-current-buffer peer-buffer (= (buffer-size) 4))))
                  (list inherited quiet empty (with-current-buffer client-buffer (buffer-string))
                        peer-quiet peer-empty (with-current-buffer peer-buffer (buffer-string))))))))
      (dolist (process (list peer client server))
        (when (and process (process-live-p process)) (delete-process process)))
      (when (and socket (file-exists-p socket)) (delete-file socket))
      (kill-buffer client-buffer) (kill-buffer peer-buffer))))
(qa (qa206-network nil nil) '(t t t "DATA" t t "PEER"))
(qa (qa206-network t nil) '(t t t "DATA" t t "PEER"))
(qa (qa206-network nil t) '(1 t t))
(qa (qa206-network t t) '(1 t t))

;; A callback deleting the process must stop later reads/callbacks.
(qa-lex
 (let ((calls 0) p)
   (unwind-protect
       (progn
         (setq p (make-process :name "qa206-delete" :noquery t :sentinel #'ignore
                              :connection-type 'pipe
                              :filter (lambda (process _string)
                                        (setq calls (1+ calls)) (delete-process process))
                              :command '("/bin/sh" "-c" "printf '%050000d' 0; read done")))
         (qa206-until p (lambda () (> calls 0)))
         (list calls (not (process-live-p p))))
     (when (and p (process-live-p p)) (delete-process p))))
 '(1 t))
(princ (qa--done)) (terpri)
