;;; -*- lexical-binding: t; -*-
(require 'timer)

(defun audit-process-drain (p)
  (let ((n 0))
    (while (and (process-live-p p) (< n 200))
      (setq n (1+ n)) (accept-process-output p 0.01))
    (accept-process-output p 0.01)))

(defun audit-process-with (command options fn)
  (let ((b (generate-new-buffer " *audit-child-output*")) p)
    (unwind-protect
        (progn
          (setq p (apply #'make-process :name "audit-child" :buffer b
                         :command command :noquery t :sentinel #'ignore options))
          (funcall fn p b))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer b))))

(defun audit-net-server (&optional options)
  (apply #'make-network-process
         (append options '(:name "audit-net-server" :host "127.0.0.1"
                           :server t :service 0 :noquery t :filter ignore :sentinel ignore))))

(defun audit-net-with-pair (options fn)
  (let ((before (process-list)) server client peer)
    (unwind-protect
        (progn
          (setq server (audit-net-server options))
          (message "NETWORK server port %S" (process-contact server :service))
          (setq client (make-network-process :name "audit-net-client" :host "127.0.0.1"
                                             :service (process-contact server :service)
                                             :noquery t :filter #'ignore :sentinel #'ignore))
          (let ((n 0))
            (while (and (not peer) (< n 50))
              (setq n (1+ n))
              (accept-process-output nil 0.01)
              (dolist (p (process-list))
                (when (and (not (memq p before)) (not (eq p server)) (not (eq p client)))
                  (setq peer p)))))
          (unless peer (error "No accepted peer in audit fixture"))
          (funcall fn server client peer))
      (dolist (p (process-list))
        (unless (memq p before) (ignore-errors (delete-process p)))))))

(qa-lex
 (audit-process-with '("/bin/sh" "-c" "printf '\\351'")
                     '(:connection-type pipe :coding iso-latin-1)
                     (lambda (p b) (audit-process-drain p)
                       (with-current-buffer b (string-to-list (buffer-string))))) '(233))
(qa-lex
 (audit-process-with '("/usr/bin/od" "-An" "-tx1")
                     '(:connection-type pipe :coding iso-latin-1)
                     (lambda (p b) (process-send-string p "é\n") (process-send-eof p)
                       (audit-process-drain p)
                       (with-current-buffer b (string-trim (buffer-string))))) "e9  0a")
(qa-err-data
 (let ((p (make-pipe-process :name "audit-coding" :noquery t)))
   (unwind-protect (set-process-coding-system p 'audit-no-such-coding 'audit-no-such-coding)
     (delete-process p))) 'coding-system-error '(audit-no-such-coding))
(qa-lex
 (audit-net-with-pair '(:coding binary)
   (lambda (server _client peer) (equal (process-coding-system server) (process-coding-system peer)))) t)

(defun qa201-metadata (coding child)
  (let (p)
    (unwind-protect
        (condition-case err
            (progn
              (setq p (if child
                          (make-process :name "qa201-meta" :command '("/bin/cat")
                                        :connection-type 'pipe :noquery t :coding coding)
                        (make-pipe-process :name "qa201-meta" :noquery t :coding coding)))
              (process-coding-system p))
          (error err))
      (when p (delete-process p)))))

(defun qa201-set (pair)
  (let ((p (make-pipe-process :name "qa201-set" :coding 'utf-8 :noquery t)))
    (unwind-protect
        (list (condition-case err (apply #'set-process-coding-system p pair) (error err))
              (process-coding-system p))
      (delete-process p))))
(qa (list (qa201-metadata nil t) (qa201-metadata nil nil)) '((utf-8-unix . utf-8-unix) (utf-8-unix . utf-8-unix)))
(qa (list (qa201-metadata 'utf-8 t) (qa201-metadata 'utf-8 nil)) '((utf-8 . utf-8) (utf-8 . utf-8)))
(qa (list (qa201-metadata 'utf-8-dos t) (qa201-metadata 'utf-8-dos nil)) '((utf-8-dos . utf-8-dos) (utf-8-dos . utf-8-dos)))
(qa (list (qa201-metadata 'iso-latin-1 t) (qa201-metadata 'iso-latin-1 nil)) '((iso-latin-1 . iso-latin-1) (iso-latin-1 . iso-latin-1)))
(qa (list (qa201-metadata 'latin-1 t) (qa201-metadata 'latin-1 nil)) '((latin-1 . latin-1) (latin-1 . latin-1)))
(qa (list (qa201-metadata 'binary t) (qa201-metadata 'binary nil)) '((binary . binary) (binary . binary)))
(qa (list (qa201-metadata 'no-conversion t) (qa201-metadata 'no-conversion nil)) '((no-conversion . no-conversion) (no-conversion . no-conversion)))
(qa (list (qa201-metadata 'raw-text t) (qa201-metadata 'raw-text nil)) '((raw-text . raw-text) (raw-text . raw-text)))
(qa (list (qa201-metadata 'utf-16le-with-signature t) (qa201-metadata 'utf-16le-with-signature nil)) '((utf-16le-with-signature . utf-16le-with-signature) (utf-16le-with-signature . utf-16le-with-signature)))
(qa (list (qa201-metadata 'shift-jis t) (qa201-metadata 'shift-jis nil)) '((coding-system-error shift-jis) (coding-system-error shift-jis)))
(qa (list (qa201-metadata 'iso-2022-jp t) (qa201-metadata 'iso-2022-jp nil)) '((iso-2022-jp . iso-2022-jp) (iso-2022-jp . iso-2022-jp)))
(qa (list (qa201-metadata 'utf-7 t) (qa201-metadata 'utf-7 nil)) '((utf-7 . utf-7) (utf-7 . utf-7)))
(qa (list (qa201-metadata '(iso-latin-1 . utf-8-dos) t) (qa201-metadata '(iso-latin-1 . utf-8-dos) nil)) '((iso-latin-1 . utf-8-dos) (iso-latin-1 . utf-8-dos)))
(qa (list (qa201-metadata '(nil . iso-latin-1) t) (qa201-metadata '(nil . iso-latin-1) nil)) '((utf-8-unix . iso-latin-1) (nil . iso-latin-1)))
(qa (list (qa201-metadata '(iso-latin-1) t) (qa201-metadata '(iso-latin-1) nil)) '((iso-latin-1 . utf-8-unix) (iso-latin-1)))
(qa (list (qa201-metadata 'qa201-invalid t) (qa201-metadata 'qa201-invalid nil)) '((coding-system-error qa201-invalid) (coding-system-error qa201-invalid)))
(qa (list (qa201-metadata "utf-8" t) (qa201-metadata "utf-8" nil)) '((wrong-type-argument symbolp "utf-8") (wrong-type-argument symbolp "utf-8")))
(qa (list (qa201-metadata 42 t) (qa201-metadata 42 nil)) '((wrong-type-argument symbolp 42) (wrong-type-argument symbolp 42)))
(qa (qa201-set '(nil nil)) '(nil (nil . raw-text-unix)))
(qa (qa201-set '(utf-8 iso-latin-1)) '(nil (utf-8 . iso-latin-1-unix)))
(qa (qa201-set '(latin-1 binary)) '(nil (latin-1 . binary)))
(qa (qa201-set '(binary binary)) '(nil (binary . binary)))
(qa (qa201-set '(utf-8-dos utf-8-dos)) '(nil (utf-8-dos . utf-8-dos)))
(qa (qa201-set '(utf-16le utf-16le)) '(nil (utf-16le . utf-16le-unix)))
(qa (qa201-set '(utf-7 utf-7)) '(nil (utf-7 . utf-7-unix)))
(qa (qa201-set '(qa201-invalid utf-8)) '((coding-system-error qa201-invalid) (utf-8 . utf-8)))
(qa (qa201-set '(iso-latin-1 qa201-invalid)) '((coding-system-error qa201-invalid) (utf-8 . utf-8)))
(qa (qa201-set '(42 utf-8)) '((wrong-type-argument symbolp 42) (utf-8 . utf-8)))
(qa (qa201-set '(utf-8 42)) '((wrong-type-argument symbolp 42) (utf-8 . utf-8)))

(defun qa201-wire (coding pieces)
  (let ((buffer (generate-new-buffer " *qa201-wire*")) p)
    (unwind-protect
        (condition-case err
            (progn
              (setq p (make-process :name "qa201-wire" :buffer buffer :noquery t
                                   :sentinel #'ignore :connection-type 'pipe :coding (cons 'utf-8-unix coding)
                                   :command '("/usr/bin/od" "-An" "-tx1")))
              (let ((before (process-coding-system p)))
                (dolist (piece pieces) (process-send-string p piece))
                (let ((sent (process-coding-system p)))
                  (process-send-eof p)
                  (let ((deadline (+ (float-time) 3)))
                    (while (and (process-live-p p) (< (float-time) deadline))
                      (accept-process-output p 0.01)))
                  (accept-process-output p 0.01)
                  (list before sent (with-current-buffer buffer (string-trim (buffer-string)))))))
          (error err))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))
(qa (qa201-wire 'iso-latin-1 '("é" "\n")) '((utf-8-unix . iso-latin-1) (utf-8-unix . iso-latin-1-unix) "e9  0a"))
(qa (qa201-wire 'utf-8 '("é" "\n")) '((utf-8-unix . utf-8) (utf-8-unix . utf-8-unix) "c3  a9  0a"))
(qa (qa201-wire 'utf-8-dos '("é" "\n")) '((utf-8-unix . utf-8-dos) (utf-8-unix . utf-8-dos) "c3  a9  0d  0a"))
(qa (qa201-wire 'utf-16le '("é" "\n")) '((utf-8-unix . utf-16le) (utf-8-unix . utf-16le-unix) "e9  00  0a"))
(qa (qa201-wire 'utf-16le-with-signature '("é" "\n")) '((utf-8-unix . utf-16le-with-signature) (utf-8-unix . utf-16le-with-signature-unix) "ff  fe  e9  00  0a"))
(qa (qa201-wire 'iso-2022-jp '("あ" "い")) '((utf-8-unix . iso-2022-jp) (utf-8-unix . iso-2022-jp-unix) "1b  24  42  24  22  24  24"))
(qa (qa201-wire 'utf-7 '("あ" "い")) '((utf-8-unix . utf-7) (utf-8-unix . utf-7-unix) "2b  4d  45  49  2b  4d  45  51"))
(qa (qa201-wire 'binary '("é" "\n")) '((utf-8-unix . binary) (utf-8-unix . binary) "c3  a9  0a"))
(qa (qa201-wire 'raw-text '("é" "\n")) '((utf-8-unix . raw-text) (utf-8-unix . raw-text-unix) "c3  a9  0a"))

(defun qa201-fragment (coding first second)
  (let ((buffer (generate-new-buffer " *qa201-fragment*")) p parts)
    (unwind-protect
        (condition-case err
            (progn
              (setq p (make-process
                       :name "qa201-fragment" :noquery t :sentinel #'ignore
                       :connection-type 'pipe :coding (cons coding 'utf-8-unix)
                       :command (list "/bin/sh" "-c"
                                      (concat "printf '" first "'; IFS= read -r line; printf '" second "'"))
                       :filter (lambda (_ text) (push (string-to-list text) parts))))
              (let ((deadline (+ (float-time) 3)))
                (while (and (not (accept-process-output p 0.01)) (< (float-time) deadline))))
              (let ((before (reverse parts)) (coding-before (process-coding-system p)))
                (process-send-string p "go\n")
                (let ((deadline (+ (float-time) 3)))
                  (while (and (process-live-p p) (< (float-time) deadline))
                    (accept-process-output p 0.01)))
                (accept-process-output p 0.01)
                (list before coding-before (reverse parts) (process-coding-system p))))
          (error (list 'error err)))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))
(qa (qa201-fragment 'utf-8 "\\303" "\\251") '(nil (utf-8 . utf-8-unix) ((233)) (utf-8 . utf-8-unix)))
(qa (qa201-fragment 'utf-8-dos "A\\015" "\\012B") '(((65)) (utf-8-dos . utf-8-unix) ((65) (10 66)) (utf-8-dos . utf-8-unix)))
(qa (qa201-fragment 'utf-16le "\\351" "\\000\\012\\000") '(nil (utf-16le . utf-8-unix) ((233 10)) (utf-16le-unix . utf-8-unix)))
(qa (qa201-fragment 'utf-16le-with-signature "\\377" "\\376\\351\\000") '(nil (utf-16le-with-signature . utf-8-unix) ((233)) (utf-16le-with-signature . utf-8-unix)))
(qa (qa201-fragment 'japanese-shift-jis "\\202" "\\240") '(nil (japanese-shift-jis . utf-8-unix) ((12354)) (japanese-shift-jis . utf-8-unix)))
(qa (qa201-fragment 'iso-2022-jp "\\033$B$" "\\042$$\\033(B") '(nil (iso-2022-jp . utf-8-unix) ((12354 12356)) (iso-2022-jp . utf-8-unix)))
(qa (qa201-fragment 'utf-7 "+ME" "I-") '(((48)) (utf-7 . utf-8-unix) ((48) (73 45)) (utf-7 . utf-8-unix)))
(qa (qa201-fragment 'chinese-hz "~{V" "P~}") '(((86)) (chinese-hz . utf-8-unix) ((86) (80 126 125)) (chinese-hz . utf-8-unix)))


(defun qa201-receive (coding unibyte filter)
  (let ((buffer (generate-new-buffer " *qa201-receive*")) p result)
    (unwind-protect
        (progn
          (when unibyte (with-current-buffer buffer (set-buffer-multibyte nil)))
          (setq p (make-process :name "qa201-receive" :buffer buffer :noquery t
                     :connection-type 'pipe :sentinel #'ignore :coding coding
                     :command (if unibyte '("/bin/sh" "-c" "printf '\\303\\251'")
                                           '("/bin/sh" "-c" "printf '\\351'"))
                     :filter (and filter
                       (lambda (_ text)
                         (setq result (list (multibyte-string-p text) (string-to-list text)
                                            (text-properties-at 0 text)))))))
          (let ((deadline (+ (float-time) 3)))
            (while (and (process-live-p p) (< (float-time) deadline))
              (accept-process-output p 0.01)))
          (accept-process-output p 0.01)
          (if filter result (with-current-buffer buffer (string-to-list (buffer-string)))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

(defun qa201-network (local)
  (let ((before (process-list)) server client peer
        (path (and local (make-temp-name (expand-file-name "qa201-socket-" temporary-file-directory)))))
    (unwind-protect
        (progn
          (setq server (apply #'make-network-process
                       (append (list :name "qa201-server" :server t :noquery t
                                :coding 'iso-latin-1 :sentinel #'ignore
                                :filter (lambda (p text) (process-put p 'text text)))
                               (if local (list :family 'local :service path)
                                 '(:host "127.0.0.1" :service 0)))))
          (setq client (apply #'make-network-process
                       (append (list :name "qa201-client" :noquery t :coding 'binary
                                     :sentinel #'ignore
                                     :filter (lambda (p text) (process-put p 'text text)))
                               (if local (list :family 'local :service path)
                                 (list :host "127.0.0.1" :service (process-contact server :service))))))
          (let ((deadline (+ (float-time) 3)))
            (while (and (not peer) (< (float-time) deadline))
              (accept-process-output nil 0.01)
              (dolist (p (process-list))
                (when (and (not (memq p before)) (not (eq p server)) (not (eq p client)))
                  (setq peer p)))))
          (unless peer (error "no accepted peer"))
          (process-send-string client (unibyte-string 233))
          (let ((deadline (+ (float-time) 3)))
            (while (and (not (process-get peer 'text)) (< (float-time) deadline))
              (accept-process-output peer 0.01)))
          (process-send-string peer "é")
          (let ((deadline (+ (float-time) 3)))
            (while (and (not (process-get client 'text)) (< (float-time) deadline))
              (accept-process-output client 0.01)))
          (list (process-coding-system peer)
                (multibyte-string-p (process-get peer 'text))
                (string-to-list (process-get peer 'text))
                (multibyte-string-p (process-get client 'text))
                (string-to-list (process-get client 'text))))
      (dolist (p (process-list))
        (unless (memq p before) (ignore-errors (delete-process p))))
      (when (and path (file-exists-p path)) (delete-file path)))))
(qa (qa201-receive 'iso-latin-1 nil t) '(t (233) (charset iso-8859-1)))
(qa (qa201-receive 'binary nil t) '(nil (233) nil))
(qa (qa201-receive 'raw-text nil t) '(nil (233) nil))
(qa (qa201-receive 'utf-8 nil t) '(t (4194281) nil))
(qa (qa201-receive 'iso-latin-1 nil nil) '(233))
(qa (qa201-receive 'binary nil nil) '(4194281))
(qa (qa201-receive 'utf-8 nil nil) '(4194281))
(qa (qa201-receive 'utf-8 t nil) '(195 169))
(qa (qa201-receive 'utf-8 t t) '(t (233) nil))
(qa (qa201-network nil) '((iso-latin-1 . iso-latin-1-unix) t (233) nil (233)))
(qa (qa201-network t) '((iso-latin-1 . iso-latin-1-unix) t (233) nil (233)))

(princ (qa--done)) (terpri)
