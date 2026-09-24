;; probe_eval12 — process coverage sweep: serial via live pty, kill/delete
;; variants, stderr split, system processes, mark accessors.
(defun pe12 (label v) (princ label) (princ "=") (princ v) (princ " "))

;; --- serial process on a live pty slave ---
(let* ((holder (make-process :name "pe12pty" :command '("cat") :noquery t))
       (tty (process-tty-name holder)))
  (pe12 'tty (stringp tty))
  (let ((sp (condition-case e
              (make-serial-process :port tty :speed 9600 :name "pe12ser")
            (error 'ser-fail))))
    (pe12 'serok (list (processp sp)
                       (and (processp sp) (process-status sp))))
    (when (processp sp)
      (pe12 'serct (process-contact sp))
      (pe12 'sercfg (condition-case e
                        (progn (serial-process-configure
                                :process sp :speed 9600 :bytesize 8
                                :parity nil :stopbits 1 :flowcontrol nil)
                               'cfg-ok)
                      (error 'cfg-err)))
      (delete-process sp)))
  (delete-process holder))

;; --- kill/delete on a real child ---
(let ((p (make-process :name "pe12k" :command '("sleep" "9") :noquery t)))
  (pe12 'killret (kill-process p))
  (accept-process-output p 1)
  (pe12 'kstat (list (process-status p) (process-exit-status p)))
  (delete-process p))

;; --- stop/continue on a real child ---
(let ((p (make-process :name "pe12sc" :command '("sleep" "9") :noquery t)))
  (pe12 'stopr (stop-process p))
  (accept-process-output p 1)
  (pe12 'stopst (process-status p))
  (pe12 'contr (continue-process p))
  (accept-process-output p 1)
  (pe12 'contst (process-status p))
  (delete-process p))

;; --- :stderr split pipe ---
(let ((p (make-process :name "pe12se"
                       :command '("sh" "-c" "echo OUT; echo ERR >&2")
                       :stderr (make-pipe-process :name "pe12serr"
                                                  :buffer (get-buffer-create
                                                           " *pe12serr*"))
                       :buffer (get-buffer-create " *pe12se*")
                       :noquery t)))
  (accept-process-output p 2)
  (accept-process-output nil 1)
  (pe12 'seout (with-current-buffer " *pe12se*" (buffer-string)))
  (pe12 'seerr (with-current-buffer " *pe12serr*" (buffer-string))))

;; --- stderr redirect to a non-pipe process errors ---
(let ((sink (make-process :name "pe12sink" :command '("cat")
                          :buffer (get-buffer-create " *pe12sink*")
                          :noquery t)))
  (pe12 'e2 (condition-case e
                (make-process :name "pe12e2"
                              :command '("sh" "-c" "echo E2 >&2")
                              :stderr sink :noquery t)
              (error 'npipe)))
  (delete-process sink))

;; --- process-mark / process-buffer / set-* roundtrips ---
(let ((p (make-process :name "pe12mk" :command '("sleep" "9") :noquery t)))
  (pe12 'pmnil (process-buffer p))
  (pe12 'mktype (markerp (process-mark p)))
  (pe12 'mkbuf (buffer-name (marker-buffer (process-mark p))))
  (pe12 'pbuf2 (buffer-name (process-buffer p)))
  (delete-process p))

;; --- process-running-child-p ---
(let ((p (make-process :name "pe12rc" :command '("sleep" "9") :noquery t)))
  (pe12 'rcp (list (process-running-child-p p)
                   (process-running-child-p "pe12rc")
                   (condition-case e (process-running-child-p 42)
                     (error 'rc-err))
                   (condition-case e (process-running-child-p)
                     (error 'rc-err))))
  (delete-process p))

;; --- system processes / attributes ---
(let ((pids (list-system-processes)))
  (pe12 'lsp (listp pids))
  (when (consp pids)
    (let ((attrs (process-attributes (car pids))))
      (pe12 'atkeys (and (consp attrs) (consp (car attrs))))
      (pe12 'atcomm (and attrs (stringp (cdr (assq 'comm attrs)))))))
  (pe12 'atself (let ((a (process-attributes (emacs-pid))))
                  (and a (consp (assq 'pid a))))))

;; --- make-process :file-handler / :connection-type ---
(pe12 'ctyp (process-contact
             (make-pipe-process :name "pe12ct") t))

;; --- delete-process on dead child is idempotent-ish ---
(let ((p (make-process :name "pe12dd" :command '("true") :noquery t)))
  (accept-process-output p 2)
  (pe12 'ddst (process-status p))
  (delete-process p)
  (pe12 'dd2 (processp p)))

;; --- interrupt/quit on child ---
(let ((p (make-process :name "pe12iq" :command '("sleep" "9") :noquery t)))
  (pe12 'intr (interrupt-process p))
  (accept-process-output p 1)
  (pe12 'iqst (process-status p))
  (pe12 'quit (condition-case e (quit-process p) (error 'qe))))

;; --- process-send-string to pty then read output ---
(let ((p (make-process :name "pe12cat" :command '("cat")
                       :buffer (get-buffer-create " *pe12cat*")
                       :noquery t)))
  (process-send-string p "xyz\n")
  (accept-process-output p 2)
  (accept-process-output nil 1)
  (pe12 'catout (with-current-buffer " *pe12cat*" (buffer-string)))
  (delete-process p))

;; --- filter function receiving output ---
(let ((got nil))
  (let ((p (make-process :name "pe12f" :command '("echo" "FILT")
                         :filter (lambda (pr s) (setq got s))
                         :noquery t)))
    (accept-process-output p 2)
    (accept-process-output nil 1)
    (pe12 'fout got)))

;; --- set-process-query-on-exit-flag / inherit flag setters ---
(let ((p (make-process :name "pe12q" :command '("sleep" "9") :noquery t)))
  (pe12 'qset (progn (set-process-query-on-exit-flag p nil)
                     (process-query-on-exit-flag p)))
  (pe12 'icset (progn (set-process-inherit-coding-system-flag p t)
                      (process-inherit-coding-system-flag p)))
  (delete-process p))

;; --- network-interface-info / network-interface-list on lo0-ish ---
(pe12 'nif (condition-case e
               (let ((i (car (network-interface-list))))
                 (and i (network-interface-info (car i))))
             (error 'nif-err)))
(pe12 'nilist (consp (network-interface-list)))

;; --- datagram process ---
(let ((d (condition-case e
             (make-network-process :name "pe12d" :type 'datagram
                                   :server t :host "127.0.0.1" :service 0
                                   :noquery t)
           (error 'dg-err))))
  (pe12 'dg (list (processp d)
                  (and (processp d) (process-type d))))
  (when (processp d) (delete-process d)))

;; --- format-network-address branches ---
(pe12 'fna (list (format-network-address nil)
                 (format-network-address nil t)
                 (format-network-address [127 0 0 1 80])))

;; --- process-datagram-address / set ---
(let ((srv (make-network-process :name "pe12gs" :server t :host "127.0.0.1"
                                 :service 0 :family 'ipv4 :noquery t)))
  (pe12 'pdad (condition-case e (process-datagram-address srv)
                (error 'pda-err)))
  (pe12 'pdset (condition-case e
                   (set-process-datagram-address srv [127 0 0 1 9])
                 (error 'pds-err)))
  (delete-process srv))

;; --- process status/type/string edge args ---
(pe12 'stnil (condition-case e (process-status nil) (error 'st-err)))
(pe12 'wnil (condition-case e (process-type nil) (error 'wt)))
(pe12 'gnil (get-process "no-such-proc"))
(pe12 'gnil2 (condition-case e (get-process nil) (error 'ge)))

(princ "\n")
