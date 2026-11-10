;;; qa_scram_iteration.el --- SCRAM iteration count validation regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/sasl-scram-rfc.el"
       (file-name-directory load-file-name)))

(defun qa-scram-reject-invalid-iteration (server-first-message)
  "Return whether SERVER-FIRST-MESSAGE is rejected before password access."
  (let* ((client (sasl-make-client
                  (get 'sasl-scram-sha-1 'sasl-mechanism)
                  "alice" "xmpp" "example.com"))
         (password-called nil)
         (hmac-called nil))
    (sasl-client-set-property client 'c-nonce "clientNonce")
    (condition-case nil
        (cl-letf (((symbol-function 'sasl-read-passphrase)
                   (lambda (_prompt)
                     (setq password-called t)
                     "password"))
                  ((symbol-function 'rfc2104-hash)
                   (lambda (&rest _arguments)
                     (setq hmac-called t)
                     (error "HMAC must not run for an invalid iteration"))))
          (sasl-scram--client-final-message
           'sha1 64 20 client (vector nil server-first-message))
          'returned)
      (sasl-error 'sasl-error))
    (list password-called hmac-called)))

(qa
 (qa-scram-reject-invalid-iteration
  "r=clientNonceServer,s=YWJj,i=0")
 '(nil nil))

(qa
 (qa-scram-reject-invalid-iteration
  "r=clientNonceServer,s=YWJj,i=01")
 '(nil nil))

(princ (qa--done))
