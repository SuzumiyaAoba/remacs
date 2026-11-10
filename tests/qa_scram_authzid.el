;;; qa_scram_authzid.el --- SCRAM GS2 authzid escaping regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/sasl-scram-rfc.el"
       (file-name-directory load-file-name)))

(qa
 (let ((client (sasl-make-client
                'sasl-scram-rfc "alice" "xmpp" "example.com")))
   (sasl-client-set-property client 'authenticator-name "admin,root=ops")
   (sasl-scram-construct-gs2-header client))
 "n,a=admin=2Croot=3Dops,")

(qa
 (sasl-scram-construct-gs2-header
  (sasl-make-client 'sasl-scram-rfc "alice" "xmpp" "example.com"))
 "n,,")

(princ (qa--done))
