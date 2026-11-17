;;; qa_mail_dont_reply_to_domain_literal.el --- address separator regression -*- lexical-binding: t -*-

(require 'mail-utils)

(qa
 (list (let ((mail-dont-reply-to-names "\\`user@\\[foo,bar\\]\\'"))
         (mail-dont-reply-to "user@[foo,bar]"))
       (let ((mail-dont-reply-to-names "\\`user@\\[foobar\\]\\'"))
         (mail-dont-reply-to "user@[foobar]"))
       (let ((mail-dont-reply-to-names "\\`user@\\[foo,bar\\]\\'"))
         (mail-dont-reply-to "user@[foo,bar],other@example")))
 '("" "" "other@example"))

(princ (qa--done))
