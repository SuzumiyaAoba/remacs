;;; qa_rfc2047_helpers_loaded.el --- RFC 2047 decoding regression -*- lexical-binding: t -*-

(require 'rfc2047)

(qa (fboundp 'rfc2047--field-body-start-p) t)
(qa (fboundp 'rfc2047--encoded-word-bounded-p) t)
(qa (rfc2047-decode-string "=?UTF-8?Q?hello?=") "hello")

(princ (qa--done))
