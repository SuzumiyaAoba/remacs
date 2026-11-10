;;; qa_scram_password.el --- SCRAM password preparation regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/sasl-scram-rfc.el"
       (file-name-directory load-file-name)))

(qa (sasl-scram--prepare-password "password") "password")

(qa
 (let ((hmac-called nil)
       (client (sasl-make-client
                (get 'sasl-scram-sha-1 'sasl-mechanism)
                "alice" "xmpp" "example.com")))
   (sasl-client-set-property client 'c-nonce "clientNonce")
   (condition-case err
       (cl-letf (((symbol-function 'sasl-read-passphrase)
                  (lambda (_prompt) (concat "pass" (string #x00a0) "word")))
                 ((symbol-function 'rfc2104-hash)
                  (lambda (&rest _arguments)
                    (setq hmac-called t)
                    (error "Unexpected HMAC"))))
         (sasl-scram--client-final-message
          #'sha1 64 20 client [nil "r=clientNonceServer,s=YWJj,i=1"]))
     (sasl-error (list 'sasl-error hmac-called))))
 '(sasl-error nil))

(princ (qa--done))
