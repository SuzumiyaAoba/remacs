;;; -*- lexical-binding: t; -*-
;; Issue #258: string ordering compares Lisp character codes, with raw
;; unibyte bytes and the current Lisp-visible symbol names.
(defun qa258-order (left right)
 (list (string-lessp left right) (string-lessp right left)
       (string< left right) (string< right left)
       (string-collate-lessp left right) (string-collate-lessp right left)))
(defun qa258-error (left right)
 (condition-case err (string-lessp left right)
  (error (list (car err) (cadr err) (eq (nth 2 err) left) (eq (nth 2 err) right)))))
(qa
(qa258-order (unibyte-string 0) (string 0))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (unibyte-string 1) (string 10))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 65) (string 97))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 127) (string 128))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 128) (string 128))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (unibyte-string 128) (string 129))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 160) (string 233))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 200) (string 201))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 233) (string 233))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (unibyte-string 233) (string 256))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 255) (string 255))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (unibyte-string 255) (string 256))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 255) (string 26085))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 233) (string 4194281))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 55295) (string 55296))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 55296) (string 57344))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 56319) (string 56320))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 57343) (string 57344))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 65535) (string 65536))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 1114111) (string 1114112))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 1114112) (string 1310720))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 1310720) (string 4194176))
'(t nil t nil t nil)
)

(qa
(qa258-order (string 4194176) (string 1114111))
'(nil t nil t nil t)
)

(qa
(qa258-order (string 4194303) (string 4194176))
'(nil t nil t nil t)
)

(qa
(qa258-order "" "a")
'(t nil t nil t nil)
)

(qa
(qa258-order "a" "aa")
'(t nil t nil t nil)
)

(qa
(qa258-order "a" "a")
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (concat "a" (unibyte-string 233))
	     (concat "a" (string 233) "z"))
'(t nil t nil t nil)
)

(qa
(qa258-order (concat "a" (string 55296)) (concat "a" (string 57344)))
'(t nil t nil t nil)
)

(qa
(qa258-order nil "nil")
'(nil nil nil nil nil nil)
)

(qa
(qa258-order 't "z")
'(t nil t nil t nil)
)

(qa
(qa258-order 'alpha 'beta)
'(t nil t nil t nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 128)) (string 128))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (unibyte-string 128) (make-symbol (string 128)))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 128))
	     (make-symbol (string 128)))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 233)) (string 233))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (unibyte-string 233) (make-symbol (string 233)))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 233))
	     (make-symbol (string 233)))
'(nil nil nil nil nil nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 233)) (string 256))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 233) (make-symbol (string 256)))
'(t nil t nil t nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 233))
	     (make-symbol (string 256)))
'(t nil t nil t nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 255)) (string 26085))
'(t nil t nil t nil)
)

(qa
(qa258-order (unibyte-string 255) (make-symbol (string 26085)))
'(t nil t nil t nil)
)

(qa
(qa258-order (make-symbol (unibyte-string 255))
	     (make-symbol (string 26085)))
'(t nil t nil t nil)
)

(qa
(let* ((name (unibyte-string 233)) (symbol (make-symbol name)))
  (aset name 0 255)
  (list (eq name (symbol-name symbol))
	(qa258-order symbol (string 240))))
'(t (nil t nil t nil t))
)

(qa
(let* ((name (copy-sequence "a")) (symbol (make-symbol name)))
  (aset name 0 122) (garbage-collect)
  (list (eq name (symbol-name symbol)) (qa258-order symbol "m")))
'(t (nil t nil t nil t))
)

(qa
(let ((ok t))
  (dotimes (code 256)
    (let ((a (unibyte-string code)) (b (string code)))
      (when (or (string-lessp a b) (string-lessp b a)) (setq ok nil))))
  ok)
't
)

(qa
(let ((ok t))
  (dotimes (code 256)
    (let ((a (unibyte-string code)) (b (string 256)))
      (unless (and (string-lessp a b) (not (string-lessp b a)))
	(setq ok nil))))
  ok)
't
)

(qa
(let ((ok t))
  (dotimes (offset 128)
    (let*
	((code (+ 128 offset)) (a (unibyte-string code))
	 (b (string (+ 4194048 code))))
      (unless (and (string-lessp a b) (not (string-lessp b a)))
	(setq ok nil))))
  ok)
't
)

(qa
(let ((ok t))
  (dotimes (offset 128)
    (let ((a (string (+ 4194176 offset))) (b (string 1114111)))
      (unless (and (not (string-lessp a b)) (string-lessp b a))
	(setq ok nil))))
  ok)
't
)

(qa
(let ((byte (unibyte-string 233)) (unicode (string 233)))
  (list (equal byte unicode) (string= byte unicode)
	(string-lessp byte unicode) (string-lessp unicode byte)
	(compare-strings byte nil nil unicode nil nil)))
'(nil nil nil nil 1)
)

(qa
(list (string-collate-lessp "A" "a" nil t)
      (string-collate-lessp (unibyte-string 233) (string 256) 'bad
			    'bad))
'(t t)
)

(qa
(qa258-error 42 24)
'(wrong-type-argument stringp t nil)
)

(qa
(qa258-error 42 "a")
'(wrong-type-argument stringp t nil)
)

(qa
(qa258-error "a" 42)
'(wrong-type-argument stringp nil t)
)

(qa
(qa258-error [] "a")
'(wrong-type-argument stringp t nil)
)

(qa
(qa258-error "a" (record 'tag 1))
'(wrong-type-argument stringp nil t)
)

(qa
(qa258-error (point-marker) "a")
'(wrong-type-argument stringp t nil)
)

(qa
(qa258-error "a" (symbol-function 'car))
'(wrong-type-argument stringp nil t)
)

(qa
(qa258-error (expt 2 100) nil)
'(wrong-type-argument stringp t nil)
)

(qa
(qa258-error nil [])
'(wrong-type-argument stringp nil t)
)

(qa
(let ((fn #[514 "\231\207" [] 4 "\n\n(fn A B)"]))
  (list (funcall fn (unibyte-string 233) (string 256))
	(funcall fn (string 256) (unibyte-string 233))
	(funcall fn (unibyte-string 233) (string 233))
	(funcall fn (string 233) (unibyte-string 233))))
'(t nil nil nil)
)

(qa
(let ((fn #[514 "\231\207" [] 4 "\n\n(fn A B)"]))
  (list (funcall fn (unibyte-string 233) (string 256))
	(funcall fn (string 256) (unibyte-string 233))
	(funcall fn (unibyte-string 233) (string 233))
	(funcall fn (string 233) (unibyte-string 233))))
'(t nil nil nil)
)


(princ (qa--done)) (terpri)
