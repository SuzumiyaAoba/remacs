;;; qa_completion_string_flags.el --- Preserve completion prefix representation -*- lexical-binding: t; -*-

(defun qa322-string (kind other)
  (cond ((eq kind 'unibyte) (unibyte-string 255 (if other 66 65)))
        ((eq kind 'multibyte-ascii) (string-to-multibyte (if other "abd" "abc")))
        ((eq kind 'unicode) (if other "a月y" "a日x"))
        ((eq kind 'multibyte-byte) (string-as-multibyte (unibyte-string 255 (if other 66 65))))
        (t (if other "abd" "abc"))))

(defun qa322-prefix (kind collection &optional partial)
  (let* ((source (qa322-string kind nil))
         (_ (put-text-property 0 (length source) 'audit 42 source))
         (items (if partial (list source (qa322-string kind t)) (list source)))
         (table (cond ((eq collection 'alist) (mapcar (lambda (s) (cons s t)) items))
                      ((eq collection 'hash)
                       (let ((hash (make-hash-table :test 'equal)))
                         (dolist (s items) (puthash s t hash)) hash))
                      (t items)))
         (result (try-completion "" table)))
    (list (multibyte-string-p result) (string-to-list result) (string-bytes result)
          (and (> (length result) 0) (get-text-property 0 'audit result))
          (multibyte-string-p source) (string-to-list source))))

;; ascii full candidate via list
(qa (qa322-prefix 'ascii 'list)
    '(nil (97 98 99) 3 42 nil (97 98 99)))

;; ascii full candidate via alist
(qa (qa322-prefix 'ascii 'alist)
    '(nil (97 98 99) 3 42 nil (97 98 99)))

;; ascii full candidate via hash
(qa (qa322-prefix 'ascii 'hash)
    '(nil (97 98 99) 3 42 nil (97 98 99)))

;; ascii common prefix
(qa (qa322-prefix 'ascii 'list t)
    '(nil (97 98) 2 42 nil (97 98 99)))

;; unibyte full candidate via list
(qa (qa322-prefix 'unibyte 'list)
    '(nil (255 65) 2 42 nil (255 65)))

;; unibyte full candidate via alist
(qa (qa322-prefix 'unibyte 'alist)
    '(nil (255 65) 2 42 nil (255 65)))

;; unibyte full candidate via hash
(qa (qa322-prefix 'unibyte 'hash)
    '(nil (255 65) 2 42 nil (255 65)))

;; unibyte common prefix
(qa (qa322-prefix 'unibyte 'list t)
    '(nil (255) 1 42 nil (255 65)))

;; multibyte-ascii full candidate via list
(qa (qa322-prefix 'multibyte-ascii 'list)
    '(t (97 98 99) 3 42 t (97 98 99)))

;; multibyte-ascii full candidate via alist
(qa (qa322-prefix 'multibyte-ascii 'alist)
    '(t (97 98 99) 3 42 t (97 98 99)))

;; multibyte-ascii full candidate via hash
(qa (qa322-prefix 'multibyte-ascii 'hash)
    '(t (97 98 99) 3 42 t (97 98 99)))

;; multibyte-ascii common prefix
(qa (qa322-prefix 'multibyte-ascii 'list t)
    '(t (97 98) 2 42 t (97 98 99)))

;; unicode full candidate via list
(qa (qa322-prefix 'unicode 'list)
    '(t (97 26085 120) 5 42 t (97 26085 120)))

;; unicode full candidate via alist
(qa (qa322-prefix 'unicode 'alist)
    '(t (97 26085 120) 5 42 t (97 26085 120)))

;; unicode full candidate via hash
(qa (qa322-prefix 'unicode 'hash)
    '(t (97 26085 120) 5 42 t (97 26085 120)))

;; unicode common prefix
(qa (qa322-prefix 'unicode 'list t)
    '(t (97) 1 42 t (97 26085 120)))

;; multibyte-byte full candidate via list
(qa (qa322-prefix 'multibyte-byte 'list)
    '(t (4194303 65) 3 42 t (4194303 65)))

;; multibyte-byte full candidate via alist
(qa (qa322-prefix 'multibyte-byte 'alist)
    '(t (4194303 65) 3 42 t (4194303 65)))

;; multibyte-byte full candidate via hash
(qa (qa322-prefix 'multibyte-byte 'hash)
    '(t (4194303 65) 3 42 t (4194303 65)))

;; multibyte-byte common prefix
(qa (qa322-prefix 'multibyte-byte 'list t)
    '(t (4194303) 2 42 t (4194303 65)))

;; empty prefix retains source representation
(qa (let* ((source (if nil (string-to-multibyte "abc") "abc")) (other (if nil (string-to-multibyte "xyz") "xyz")) (result (try-completion "" (list source other)))) (list (multibyte-string-p result) (length result)))
    '(nil 0))

;; unique exact match still returns t
(qa (let ((source (if nil (string-to-multibyte "abc") "abc"))) (try-completion source (list source)))
    't)

;; empty prefix retains source representation
(qa (let* ((source (if t (string-to-multibyte "abc") "abc")) (other (if t (string-to-multibyte "xyz") "xyz")) (result (try-completion "" (list source other)))) (list (multibyte-string-p result) (length result)))
    '(t 0))

;; unique exact match still returns t
(qa (let ((source (if t (string-to-multibyte "abc") "abc"))) (try-completion source (list source)))
    't)

(princ (qa--done))
