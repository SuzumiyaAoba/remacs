;;; -*- lexical-binding: nil; -*-
;; #624: actual private-use Unicode is not an internal character proxy.
(dolist (code '(#xf0080 #xf0100 #xf0a80 #xf0b00 #xf0c80 #xf0d80))
  (qa (let ((text (string code)))
        (list (equal (string-to-list (json-parse-string (concat "\"" text "\""))) (list code))
              (equal (string-to-list (json-parse-string (json-serialize text))) (list code))
              (with-temp-buffer
                (insert (concat "\"" text "\""))
                (goto-char (point-min))
                (equal (string-to-list (json-parse-buffer)) (list code)))
              (let ((object (make-hash-table :test 'equal)))
                (puthash text text object)
                (equal (gethash text (json-parse-string (json-serialize object))) text))
              (equal (gethash text (json-parse-string
                                    (json-serialize (list (cons (intern text) text))))) text)))
      '(t t t t t)))
(dolist (code '(#xd800 #x110000 #x200000 #x3fff80))
  (qa (let ((text (string code)))
        (list (condition-case error
                  (json-parse-string (concat "\"" text "\""))
                (error (car error)))
              (condition-case error (json-serialize text)
                (error (car error)))
              (with-temp-buffer
                (insert (concat "\"" text "\""))
                (goto-char (point-min))
                (condition-case error (json-parse-buffer) (error (car error))))))
      '(json-utf8-decode-error wrong-type-argument json-utf8-decode-error)))
(princ (qa--done))
