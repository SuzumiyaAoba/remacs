;;; qa_rfc2047_q_underscore.el --- RFC 2047 Q underscore regression -*- lexical-binding: t -*-

(require 'rfc2047)

(let ((rfc2047-encoding-type 'address-mime))
  (qa (rfc2047-encode-string "Name_é <x@example.com>")
      "=?utf-8?Q?Name=5F=C3=A9?= <x@example.com>")
  (qa (rfc2047-decode-string "=?utf-8?Q?Name=5F=C3=A9?= <x@example.com>")
      "Name_é <x@example.com>"))

(princ (qa--done))
