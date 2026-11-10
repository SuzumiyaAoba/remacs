;;; qa_scram_username.el --- SCRAM username escaping regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/sasl-scram-rfc.el"
       (file-name-directory load-file-name)))

(qa
 (sasl-scram--client-first-message-bare
  (sasl-make-client 'sasl-scram-rfc "alice,admin=ops" "xmpp" "example.com"))
 "n=alice=2Cadmin=3Dops,r=")

(qa
 (sasl-scram--client-first-message-bare
  (sasl-make-client 'sasl-scram-rfc "alice=2Cadmin" "xmpp" "example.com"))
 "n=alice=3D2Cadmin,r=")

(qa
 (sasl-scram--client-first-message-bare
  (sasl-make-client 'sasl-scram-rfc "alice" "xmpp" "example.com"))
 "n=alice,r=")

(princ (qa--done))
