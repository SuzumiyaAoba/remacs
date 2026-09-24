;; probe_eval11 — process internals + charset details.
(defun pe11 (label v) (princ label) (princ "=") (princ v) (princ " "))

;; --- network processes ---
(let ((srv (make-network-process :name "pe11s" :server t :host "127.0.0.1"
                                 :service 0 :family 'ipv4
                                 :buffer (get-buffer-create " *pe11s*"))))
  (pe11 'nsrv (list (process-status srv) (process-type srv) (processp srv)))
  (let ((port (cadr (process-contact srv))))
    (let ((cl (make-network-process :name "pe11c" :host "127.0.0.1"
                                    :service port :family 'ipv4
                                    :buffer (get-buffer-create " *pe11c*"))))
      (accept-process-output cl 2)
      (pe11 'ncl (list (process-status cl) (process-type cl)
                       (car (process-contact cl))))
      (accept-process-output srv 2)
      (process-send-string cl "ping")
      (accept-process-output srv 1)
      (accept-process-output nil 1)
      (pe11 'nbuf (with-current-buffer " *pe11s*" (buffer-string)))
      (pe11 'ncmd (process-command cl))
      (delete-process cl)
      (delete-process srv))))

;; --- process accessors / misc ---
(let ((p (make-process :name "pe11a" :command '("sleep" "9")
                       :buffer (get-buffer-create " *pe11a*"))))
  (pe11 'pcmd (car (process-command p)))
  (pe11 'pfilt (progn (set-process-filter p 'internal-default-process-filter)
                      (process-filter p)))
  (pe11 'psent (progn (set-process-sentinel p 'ignore) (process-sentinel p)))
  (pe11 'pbuf (progn (set-process-buffer p (get-buffer-create " *pe11b*"))
                     (buffer-name (process-buffer p))))
  (pe11 'plist2 (process-plist p))
  (pe11 'ppid (integerp (process-id p)))
  (pe11 'plistv (memq p (process-list)))
  (pe11 'psreg (progn (with-temp-buffer (insert "REGION") (process-send-region p 1 4))
                      (process-status p)))
  (pe11 'peof (process-send-eof p))
  (pe11 'pquery2 (process-query-on-exit-flag p))
  (pe11 'pquit (quit-process p))
  (delete-process p))

;; --- signal-process variants ---
(let ((p (make-process :name "pe11sig" :command '("sleep" "9"))))
  (pe11 'sigusr (signal-process p 'SIGUSR1))
  (accept-process-output p 1)
  (pe11 'sigstat (process-status p))
  (let ((p2 (make-process :name "pe11sig2" :command '("sleep" "9"))))
    (pe11 'sighup (signal-process p2 'SIGHUP))
    (accept-process-output p2 1)
    (pe11 'hupstat (list (process-status p2) (process-exit-status p2))))
  (pe11 'signo (signal-process 999999 'SIGTERM)))

;; --- serial process (error path) ---
(pe11 'ser (condition-case e
               (make-serial-process :port "/nonexistent-tty-xyz" :speed 9600)
             (error 'ser-err)))

;; --- accept variants ---
(let ((p (make-process :name "pe11ac" :command '("sh" "-c" "sleep 0.2; echo late")
                       :buffer (get-buffer-create " *pe11ac*"))))
  (pe11 'ac4 (accept-process-output p 1 0 nil))
  (accept-process-output p 2)
  (pe11 'acbuf (with-current-buffer " *pe11ac*" (buffer-string)))
  (delete-process p))
(pe11 'ac0 (accept-process-output))
(pe11 'acn (accept-process-output nil 0 50))

;; --- process-lines / call-process paths ---
(pe11 'plw (process-lines "sh" "-c" "printf 'x\\ny\\n'"))
(pe11 'ple (condition-case e (process-lines "sh" "-c" "exit 2") (error 'pl-err)))
(pe11 'pli2 (process-lines-ignore-status "sh" "-c" "echo q; exit 2"))

;; --- process attributes / tty ---
(pe11 'pat2 (let ((pa (process-attributes (emacs-pid))))
              (and (consp pa) (consp (assq 'euid pa)) (consp (assq 'user pa))
                   (consp (assq 'comm pa)))))
(pe11 'ptty (process-tty-name (make-pipe-process :name "pe11tty")))

;; --- char code property machinery ---
(pe11 'ccp1 (list (char-code-property-description 'general-category 'Lu)
                  (char-code-property-description 'general-category 'Ll)
                  (char-code-property-description 'bidi-class 'L)
                  (char-code-property-description 'nosuch-prop-xyz 'foo)))
(pe11 'ccp2 (progn (let ((tbl (make-char-table 'char-code-property-table)))
                     (set-char-table-extra-slot tbl 0 'my-prop2)
                     (define-char-code-property 'my-prop2 tbl))
                   (put-char-code-property ?a 'my-prop2 'VA)
                   (put-char-code-property ?b 'my-prop2 'VB)
                   (list (get-char-code-property ?a 'my-prop2)
                         (get-char-code-property ?b 'my-prop2)
                         (get-char-code-property ?c 'my-prop2))))
(pe11 'ccp3 (progn (let ((tbl (make-char-table 'char-code-property-table)))
                     (set-char-table-extra-slot tbl 0 'my-prop3)
                     (define-char-code-property 'my-prop3 tbl))
                   (put-char-code-property ?x 'my-prop3 7)
                   (get-char-code-property ?x 'my-prop3)))

;; --- coding-system-type on many names ---
(pe11 'csty (mapcar #'coding-system-type
                    '(utf-8 utf-8-unix utf-8-dos utf-8-mac no-conversion
                      raw-text undecided latin-1 iso-8859-1 binary
                      cn-gb-2312 japanese-shift-jis us-ascii euc-jp
                      emacs-mule cp1251)))
(pe11 'csty2 (condition-case e (coding-system-type 'chinese-gb2312)
               (coding-system-error 'cs-err)))

;; --- translation tables ---
(pe11 'tt1 (progn (define-translation-table 'tt-a '((?a . ?b)))
                  (char-table-p (get 'tt-a 'translation-table))))
(pe11 'tt2 (char-table-p (make-translation-table-from-vector
                          (make-vector 256 ?x))))
(pe11 'tt3 (char-table-p (make-translation-table-from-alist '((?x . ?y)))))

;; --- charset-after ---
(pe11 'ca2 (with-temp-buffer (insert "xy") (list (charset-after) (charset-after 2))))

;; --- define-charset error paths ---
(pe11 'dcv (condition-case e (define-charset 'bad1 "not-a-vector")
             (wrong-type-argument 'wt) (error (car e))))
(pe11 'dcn (condition-case e (define-charset 42 [1 2])
             (wrong-type-argument 'wt) (error (car e))))

;; --- find-charset-region multibyte ---
(pe11 'fcr2 (with-temp-buffer (insert "a\x00e9z") (find-charset-region 1 4)))

;; --- format-network-address variants ---
(pe11 'fna4 (list (format-network-address [10 0 0 1 80])
                  (condition-case e (format-network-address '(9 9 9 9))
                    (error 'fna-err))
                  (format-network-address nil)))

;; --- network-interface-info shape ---
(pe11 'nii2 (let ((ni (network-interface-info "lo0")))
              (and (consp ni) (consp (car ni)))))

;; --- process-datagram-address / misc ---
(pe11 'pda (condition-case e (process-datagram-address (make-pipe-process :name "pe11dg"))
             (error 'pda-err)))

;; --- coverage: filters, marks, plists ---
(pe11 'idf (let ((p (make-process :name "pe11idf" :command '("echo" "IDFOUT")
                                  :filter 'internal-default-process-filter
                                  :buffer (get-buffer-create " *pe11idf*"))))
             (accept-process-output p 2)
             (with-current-buffer " *pe11idf*" (buffer-string))))
(pe11 'lp (list-processes))
(pe11 'pm2 (let ((p (make-pipe-process :name "pe11pm")))
             (markerp (process-mark p))))
(pe11 'pp2 (let ((p (make-pipe-process :name "pe11pp")))
             (list (car (process-put p 'k 'v)) (process-get p 'k) (process-get p 'z))))
(pe11 'spb (let ((p (make-pipe-process :name "pe11spb")))
             (set-process-buffer p (get-buffer-create " *pe11spb*"))
             (buffer-name (process-buffer p))))
(pe11 'pcq (let ((p (make-pipe-process :name "pe11pcq" :noquery t)))
             (list (process-query-on-exit-flag p)
                   (progn (set-process-query-on-exit-flag p t)
                          (process-query-on-exit-flag p)))))
(pe11 'pwq (let ((p (make-pipe-process :name "pe11pwq" :noquery t)))
             (list (process-status p) (processp p))))
(pe11 'mkct (process-type (make-process :name "pe11ct" :command '("sleep" "1")
                                        :connection-type 'pty :noquery t)))
(pe11 'mkfh (processp (make-process :name "pe11fh" :command '("sleep" "1")
                                    :file-handler t :noquery t)))
(pe11 'mknc (processp (make-process :name "pe11nc")))
(pe11 'mknc2 (processp (make-process :name "pe11nc2" :command nil)))
(pe11 'mkstop (condition-case e (make-process :name "pe11st" :command '("sleep" "5")
                                              :stop t :noquery t)
                (wrong-type-argument 'wt-stop) (error 'er)))
(pe11 'mkstop2 (process-status (make-pipe-process :name "pe11st2" :stop t)))
(pe11 'serr (let ((p (make-process :name "pe11se" :command '("sh" "-c" "echo o; echo e >&2")
                                   :buffer (get-buffer-create " *pe11o*")
                                   :stderr (get-buffer-create " *pe11e*")
                                   :noquery t)))
             (accept-process-output p 2)
             (list (with-current-buffer " *pe11o*" (buffer-string))
                   (with-current-buffer " *pe11e*" (buffer-string)))))

;; --- signals by name ---
(pe11 'sign (let ((p (make-process :name "pe11sn" :command '("sleep" "5") :noquery t)))
              (signal-process p 'SIGTERM)
              (accept-process-output p 2)
              (process-status p)))
(pe11 'sign2 (let ((p (make-process :name "pe11sn2" :command '("sleep" "5") :noquery t)))
               (signal-process p 'SIGHUP)
               (accept-process-output p 2)
               (process-status p)))
(pe11 'sigbad (condition-case e (signal-process (make-pipe-process :name "pe11sb") 'NOSUCHSIG)
                (error 'sig-err)))

;; --- network service names / address formats ---
(pe11 'svn (condition-case e (make-network-process :name "pe11svn" :host "127.0.0.1"
                                                   :service "http")
             (file-error 'fe) (error 'er)))
(pe11 'fna6 (list (format-network-address [0 0 0 0 0 0 0 1 80])
                  (format-network-address [10 0 0 1 80] t)
                  (format-network-address [10 0 0 1])))
(pe11 'nil2 (consp (network-interface-list)))
(pe11 'niia (consp (network-interface-info "lo0")))

;; --- serial process errors ---
(pe11 'ser1 (make-serial-process))
(pe11 'ser2 (condition-case e (make-serial-process :port "/nonexistent-xyz")
              (error 'ser-err)))
(pe11 'ser3 (condition-case e (make-serial-process :port "/dev/null" :speed 9600)
              (file-error 'ferr) (error 'err)))
(pe11 'serc (condition-case e (serial-process-configure :port "/dev/null" :speed 9600)
              (error 'serc-err)))

;; --- misc single-arg fns ---
(pe11 'pidnm (integerp (process-id (make-process :name "pe11id" :command '("sleep" "5")
                                                 :noquery t))))
(pe11 'pt2 (process-tty-name (make-process :name "pe11t" :command '("sleep" "5")
                                           :noquery t)))
(pe11 'pp3 (processp 'not-a-proc))
(pe11 'gp (condition-case e (get-process "no-such-proc") (error 'gp-nil)))
(pe11 'gp2 (processp (get-process "pe11pm")))
(pe11 'pcs (let ((p (make-pipe-process :name "pe11pcs")))
             (list (process-coding-system p)
                   (progn (set-process-coding-system p 'utf-8 'latin-1)
                          (process-coding-system p)))))
(pe11 'pii (let ((p (make-pipe-process :name "pe11pii")))
             (list (process-inherit-coding-system-flag p)
                   (progn (set-process-inherit-coding-system-flag p t)
                          (process-inherit-coding-system-flag p)))))
(pe11 'stty (condition-case e (process-send-string (make-pipe-process :name "pe11ss") 42)
              (wrong-type-argument 'wt) (error 'ee)))
(pe11 'kdel (let ((p (make-pipe-process :name "pe11kd")))
              (condition-case e (progn (kill-process p) 'killed)
                (error 'kill-err))))
(pe11 'del2 (let ((p (make-pipe-process :name "pe11d2")))
              (delete-process p)
              (condition-case e (delete-process p) (error 'del-err))))
(pe11 'evs (let ((seen nil))
             (let ((p (make-process :name "pe11ev" :command '("true")
                                    :sentinel (lambda (pr ev) (setq seen ev)))))
               (accept-process-output p 2)
               seen)))
(pe11 'evk (let ((seen nil))
             (let ((p (make-process :name "pe11ek" :command '("sleep" "5")
                                    :sentinel (lambda (pr ev) (setq seen ev)))))
               (delete-process p)
               seen)))
(pe11 'run2 (let ((p (make-process :name "pe11r2" :command '("sh" "-c" "sleep 0.2; echo done")
                                   :buffer (get-buffer-create " *pe11r2*")
                                   :noquery t)))
              (accept-process-output p 3)
              (list (process-status p) (process-exit-status p)
                    (with-current-buffer " *pe11r2*" (buffer-string)))))
(pe11 'apoms (condition-case e (accept-process-output nil 0.05 0)
              (wrong-type-argument 'wt) (error 'ee)))
(pe11 'apms (let ((p (make-process :name "pe11am" :command '("sleep" "5") :noquery t)))
              (list (accept-process-output p 0 50 t)
                    (progn (delete-process p) 'ok))))
(pe11 'pwm (let ((p (make-pipe-process :name "pe11wm" :buffer
                                       (get-buffer-create " *pe11wm*"))))
             (with-temp-buffer (insert "HELLO") (process-send-region p 1 6))
             (accept-process-output p 1)
             (with-current-buffer " *pe11wm*" (buffer-string))))
(princ "\n")
