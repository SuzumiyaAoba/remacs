;;; qa_json_insert_bytes.el --- UTF-8 insertion into unibyte buffers -*- lexical-binding: t; -*-
;; Verify byte contents, point and change-hook extents after buffer preparation.

(defun qa305-insert (value multibyte flip &rest options)
  (with-temp-buffer
    (set-buffer-multibyte multibyte)
    (insert "seed")
    (goto-char 2)
    (let ((changes nil)
          (before-change-functions
           (and flip (list (lambda (&rest _) (set-buffer-multibyte (not multibyte)))))))
      (let ((after-change-functions
             (list (lambda (begin end old) (push (list begin end old) changes)))))
        (list (apply #'json-insert value options)
              (string-to-list (buffer-string)) (point)
              enable-multibyte-characters (nreverse changes))))))

;; unibyte Latin-1
(qa (qa305-insert "é" nil nil)
    '(nil (115 34 195 169 34 101 101 100) 6 nil ((2 6 0))))

;; multibyte Latin-1
(qa (qa305-insert "é" t nil)
    '(nil (115 34 233 34 101 101 100) 5 t ((2 5 0))))

;; unibyte CJK
(qa (qa305-insert "日本語" nil nil)
    '(nil (115 34 230 151 165 230 156 172 232 170 158 34 101 101 100) 13 nil ((2 13 0))))

;; unibyte emoji
(qa (qa305-insert "😀" nil nil)
    '(nil (115 34 240 159 152 128 34 101 101 100) 8 nil ((2 8 0))))

;; multibyte emoji
(qa (qa305-insert "😀" t nil)
    '(nil (115 34 128512 34 101 101 100) 5 t ((2 5 0))))

;; unibyte ASCII
(qa (qa305-insert "ASCII" nil nil)
    '(nil (115 34 65 83 67 73 73 34 101 101 100) 9 nil ((2 9 0))))

;; unibyte number
(qa (qa305-insert 42 nil nil)
    '(nil (115 52 50 101 101 100) 4 nil ((2 4 0))))

;; unibyte nested values
(qa (qa305-insert ["é" "日本" "😀"] nil nil)
    '(nil (115 91 34 195 169 34 44 34 230 151 165 230 156 172 34 44 34 240 159 152 128 34 93 101 101 100) 24 nil ((2 24 0))))

;; unibyte Unicode keys
(qa (let ((table (make-hash-table :test 'equal))) (puthash "é" "日本" table) (qa305-insert table nil nil))
    '(nil (115 123 34 195 169 34 58 34 230 151 165 230 156 172 34 125 101 101 100) 17 nil ((2 17 0))))

;; unibyte null option
(qa (qa305-insert [nil] nil nil :null-object nil)
    '(nil (115 91 110 117 108 108 93 101 101 100) 8 nil ((2 8 0))))

;; hook changes to multibyte
(qa (qa305-insert "é" nil t)
    '(nil (115 34 233 34 101 101 100) 5 t ((2 5 0))))

;; hook changes to unibyte
(qa (qa305-insert "é" t t)
    '(nil (115 34 195 169 34 101 101 100) 6 nil ((2 6 0))))

;; byte serialization parity
(qa (with-temp-buffer (set-buffer-multibyte nil) (json-insert "é日本😀") (list (equal (buffer-string) (json-serialize "é日本😀")) (point) (buffer-size)))
    '(t 15 14))

;; unibyte round trip
(qa (with-temp-buffer (set-buffer-multibyte nil) (json-insert "é日本😀") (goto-char 1) (list (string-to-list (json-parse-buffer)) (point) (point-max)))
    '((233 26085 26412 128512) 15 15))

(princ (qa--done))
