;;; -*- lexical-binding: nil; -*-
;; #635 #636 #637, GNU 31.1 verified.
(defun qa-crypto-buffer-hex (spec)
  (mapconcat (lambda (byte) (format "%02x" byte))
             (string-to-list (gnutls-hash-digest 'SHA256 spec)) ""))
(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (insert (unibyte-string 128))
      (qa-crypto-buffer-hex (list (current-buffer))))
    "76be8b528d0075f7aae98d6fa57a6d3c83ae480a8469e668d7b0af968995ac71")
(qa (with-temp-buffer
      (insert "abc")
      (qa-crypto-buffer-hex (list (current-buffer) 1 2)))
    "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb")
(qa (with-temp-buffer
      (insert "abc")
      (qa-crypto-buffer-hex (list (current-buffer) 3 1)))
    "fb8e20fc2e4c3f248c60c39bd652f3c1347298bb977b8b4d5903b85055620603")
(qa (qa-crypto-buffer-hex '("abc" 1 2))
    "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d")
(qa (with-temp-buffer
      (insert "abc")
      (narrow-to-region 2 3)
      (qa-crypto-buffer-hex (list (current-buffer))))
    "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d")
(qa (with-temp-buffer
      (insert "abc")
      (condition-case error (gnutls-hash-digest 'SHA256 (list (current-buffer) 0 1))
        (error (car error))))
    'args-out-of-range)
(qa (let ((spec (list "abc")))
      (setcdr spec spec)
      (condition-case error (gnutls-hash-digest 'SHA256 spec) (error error)))
    '(wrong-type-argument integerp "abc"))
(princ (qa--done))
