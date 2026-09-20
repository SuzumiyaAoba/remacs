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
(princ "\n")
