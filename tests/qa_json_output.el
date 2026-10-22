;;; qa_json_output.el --- JSON output rejects non-Unicode strings -*- lexical-binding: t; -*-
;; Check error datum identity and atomic insertion as well as Unicode output.

(defun qa304-check (value bad &rest options)
  (list
   (condition-case error
       (list 'ok (string-to-list (apply #'json-serialize value options)))
     (error (list (car error) (cadr error) (eq (caddr error) bad))))
   (with-temp-buffer
     (insert "seed")
     (goto-char 2)
     (list
      (condition-case error
          (progn (apply #'json-insert value options) 'ok)
        (error (list (car error) (cadr error) (eq (caddr error) bad))))
      (string-to-list (buffer-string)) (point)))))

;; ASCII unibyte
(qa (qa304-check (unibyte-string 0 1 34 92 127) nil)
    '((ok (34 92 117 48 48 48 48 92 117 48 48 48 49 92 34 92 92 127 34)) (ok (115 34 92 117 48 48 48 48 92 117 48 48 48 49 92 34 92 92 127 34 101 101 100) 21)))

;; UTF-8 in unibyte string
(qa (let ((bad (unibyte-string 195 169))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; invalid byte FF
(qa (let ((bad (unibyte-string 255))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; unibyte high byte
(qa (let ((bad (unibyte-string 128))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; Unicode Latin-1
(qa (qa304-check (string 233) nil)
    '((ok (34 195 169 34)) (ok (115 34 233 34 101 101 100) 5)))

;; Unicode emoji
(qa (qa304-check (string #x1f600) nil)
    '((ok (34 240 159 152 128 34)) (ok (115 34 128512 34 101 101 100) 5)))

;; Unicode maximum
(qa (qa304-check (string #x10ffff) nil)
    '((ok (34 244 143 191 191 34)) (ok (115 34 1114111 34 101 101 100) 5)))

;; high surrogate
(qa (let ((bad (string #xd800))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; low surrogate
(qa (let ((bad (string #xdfff))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; non-Unicode Mule character
(qa (let ((bad (string #x110000))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; multibyte raw byte
(qa (let ((bad (concat "λ" (unibyte-string 255)))) (qa304-check bad bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; vector value
(qa (let ((bad (unibyte-string 255))) (qa304-check (vector bad) bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; plist value
(qa (let ((bad (unibyte-string 255))) (qa304-check (list :x bad) bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; hash value
(qa (let ((bad (unibyte-string 255)) (table (make-hash-table))) (puthash "x" bad table) (qa304-check table bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; hash key
(qa (let ((bad (unibyte-string 255)) (table (make-hash-table))) (puthash bad 1 table) (qa304-check table bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; alist symbol name
(qa (let* ((bad (unibyte-string 255)) (key (make-symbol bad))) (qa304-check (list (cons key 1)) bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; plist symbol name
(qa (let* ((bad (unibyte-string 58 255)) (key (make-symbol bad))) (qa304-check (list key 1) bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; non-Unicode symbol name
(qa (let* ((bad (string #xd800)) (key (make-symbol bad))) (qa304-check (list (cons key 1)) bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

;; configured null sentinel
(qa (let ((bad (unibyte-string 255))) (qa304-check bad nil :null-object bad))
    '((ok (110 117 108 108)) (ok (115 110 117 108 108 101 101 100) 6)))

;; configured false sentinel
(qa (let ((bad (unibyte-string 255))) (qa304-check bad nil :false-object bad))
    '((ok (102 97 108 115 101)) (ok (115 102 97 108 115 101 101 101 100) 7)))

;; key is not a sentinel value
(qa (let ((bad (unibyte-string 255)) (table (make-hash-table))) (puthash bad 1 table) (qa304-check table bad :null-object bad))
    '((wrong-type-argument json-value-p t) ((wrong-type-argument json-value-p t) (115 101 101 100) 2)))

(princ (qa--done))
