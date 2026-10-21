;;; -*- lexical-binding: t; -*-
;; Issue #252: explicit composition ranges remain one glyph, with the
;; source byte representation and text properties copied by substring.
(defun qa252-pieces (s)
  (mapcar (lambda (piece)
            (list (string-to-list piece) (multibyte-string-p piece)))
          (string-glyph-split s)))
(qa (with-temp-buffer (insert (make-string 32 120)) (qa252-pieces ""))
    'nil)

(qa
 (with-temp-buffer (insert (make-string 32 120)) (qa252-pieces "a"))
 '(((97) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120)) (qa252-pieces "abcd"))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (qa252-pieces (string-make-multibyte "abcd")))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120)) (qa252-pieces "日éabc"))
 '(((26085) t) ((233) t) ((97) t) ((98) t) ((99) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (qa252-pieces (unibyte-string 0 127 128 255)))
 '(((0) nil) ((127) nil) ((128) nil) ((255) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (qa252-pieces (string 55296 1114112 4194176 97)))
 '(((55296) t) ((1114112) t) ((4194176) t) ((97) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120)) (qa252-pieces "é🇯🇵"))
 '(((101) t) ((769) t) ((127471) t) ((127477) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 0 1) (qa252-pieces s)))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil) ((101) nil)
   ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 0 2) (qa252-pieces s)))
 '(((97 98) nil) ((99) nil) ((100) nil) ((101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 1 3) (qa252-pieces s)))
 '(((97) nil) ((98 99) nil) ((100) nil) ((101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 1 5) (qa252-pieces s)))
 '(((97) nil) ((98 99 100 101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 4 6) (qa252-pieces s)))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil) ((101 102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 0 6) (qa252-pieces s)))
 '(((97 98 99 100 101 102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (string-make-multibyte "abcdef"))))
     (compose-string s 0 1) (qa252-pieces s)))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil) ((101) nil)
   ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (string-make-multibyte "abcdef"))))
     (compose-string s 0 2) (qa252-pieces s)))
 '(((97 98) nil) ((99) nil) ((100) nil) ((101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (string-make-multibyte "abcdef"))))
     (compose-string s 1 3) (qa252-pieces s)))
 '(((97) nil) ((98 99) nil) ((100) nil) ((101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (string-make-multibyte "abcdef"))))
     (compose-string s 1 5) (qa252-pieces s)))
 '(((97) nil) ((98 99 100 101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (string-make-multibyte "abcdef"))))
     (compose-string s 4 6) (qa252-pieces s)))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil) ((101 102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (string-make-multibyte "abcdef"))))
     (compose-string s 0 6) (qa252-pieces s)))
 '(((97 98 99 100 101 102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "日本語éab")))
     (compose-string s 0 1) (qa252-pieces s)))
 '(((26085) t) ((26412) t) ((35486) t) ((233) t) ((97) t) ((98) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "日本語éab")))
     (compose-string s 0 2) (qa252-pieces s)))
 '(((26085 26412) t) ((35486) t) ((233) t) ((97) t) ((98) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "日本語éab")))
     (compose-string s 1 3) (qa252-pieces s)))
 '(((26085) t) ((26412 35486) t) ((233) t) ((97) t) ((98) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "日本語éab")))
     (compose-string s 1 5) (qa252-pieces s)))
 '(((26085) t) ((26412 35486 233 97) t) ((98) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "日本語éab")))
     (compose-string s 4 6) (qa252-pieces s)))
 '(((26085) t) ((26412) t) ((35486) t) ((233) t) ((97 98) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "日本語éab")))
     (compose-string s 0 6) (qa252-pieces s)))
 '(((26085 26412 35486 233 97 98) t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (unibyte-string 0 127 128 254 255 10))))
     (compose-string s 0 1) (qa252-pieces s)))
 '(((0) nil) ((127) nil) ((128) nil) ((254) nil) ((255) nil)
   ((10) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (unibyte-string 0 127 128 254 255 10))))
     (compose-string s 0 2) (qa252-pieces s)))
 '(((0 127) nil) ((128) nil) ((254) nil) ((255) nil) ((10) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (unibyte-string 0 127 128 254 255 10))))
     (compose-string s 1 3) (qa252-pieces s)))
 '(((0) nil) ((127 128) nil) ((254) nil) ((255) nil) ((10) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (unibyte-string 0 127 128 254 255 10))))
     (compose-string s 1 5) (qa252-pieces s)))
 '(((0) nil) ((127 128 254 255) nil) ((10) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (unibyte-string 0 127 128 254 255 10))))
     (compose-string s 4 6) (qa252-pieces s)))
 '(((0) nil) ((127) nil) ((128) nil) ((254) nil) ((255 10) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence (unibyte-string 0 127 128 254 255 10))))
     (compose-string s 0 6) (qa252-pieces s)))
 '(((0 127 128 254 255 10) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 0 2) (compose-string s 2 4)
     (compose-string s 4 6) (qa252-pieces s)))
 '(((97 98) nil) ((99 100) nil) ((101 102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "abcdef")))
     (compose-string s 0 3) (compose-string s 2 5) (qa252-pieces s)))
 '(((97 98) nil) ((99 100 101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let* ((s (copy-sequence "abcdef")) (comp (list (cons 2 nil))))
     (put-text-property 1 3 'composition comp s)
     (put-text-property 3 5 'composition (copy-tree comp) s)
     (qa252-pieces s)))
 '(((97) nil) ((98 99) nil) ((100 101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let* ((s (copy-sequence "abcdef")) (comp (list (cons 4 nil))))
     (put-text-property 1 5 'composition comp s)
     (put-text-property 2 4 'face 'bold s) (qa252-pieces s)))
 '(((97) nil) ((98 99 100 101) nil) ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let* ((s (copy-sequence "abcdef")))
     (compose-string s 1 5) (put-text-property 2 4 'composition nil s)
     (qa252-pieces s)))
 '(((97) nil) ((98) nil) ((99) nil) ((100) nil) ((101) nil)
   ((102) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let*
       ((s
	 (propertize (copy-sequence "abcd") 'face 'bold 'qa252 'value))
	(_ (compose-string s 1 3)) (pieces (string-glyph-split s)))
     (list (mapcar (lambda (p) (get-text-property 0 'face p)) pieces)
	   (mapcar (lambda (p) (get-text-property 0 'qa252 p)) pieces)
	   (equal (get-text-property 0 'composition (nth 1 pieces))
		  (get-text-property 1 'composition s))
	   (eq (car pieces) s)
	   (progn
	     (put-text-property 0 1 'face 'italic (car pieces))
	     (get-text-property 0 'face s)))))
 '((bold bold bold) (value value value) t nil bold))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let*
       ((s (copy-sequence "ab")) (_ (compose-string s 0 2))
	(pieces (string-glyph-split s)))
     (list (length pieces) (eq (car pieces) s)
	   (progn
	     (aset (car pieces) 0 122) (substring-no-properties s))
	   (substring-no-properties (car pieces)))))
 '(1 nil "ab" "zb"))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split 42) (error err)))
 '(wrong-type-argument sequencep 42))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split nil) (error err)))
 'nil)

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split [1 2]) (error err)))
 '(wrong-type-argument stringp [1 2]))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition nil s) (qa252-pieces s)))
 '(((120) nil) ((97) nil) ((98) nil) ((99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition t s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition 1 s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition 'bad s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition '(2) s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition '((2)) s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (put-text-property 1 4 'composition '((9)) s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split []) (error err)))
 'nil)

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split (bool-vector)) (error err)))
 'nil)

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split (record 'tag)) (error err)))
 '(wrong-type-argument stringp #s(tag)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split '(1 2)) (error err)))
 '(wrong-type-argument stringp (1 2)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (condition-case err (string-glyph-split '(1 . 2)) (error err)))
 '(wrong-type-argument listp 2))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let
       ((s (copy-sequence "xabcy"))
	(cat (make-symbol "qa252-category")))
     (put cat 'composition (list (cons 3 nil)))
     (put-text-property 1 4 'category cat s) (qa252-pieces s)))
 '(((120) nil) ((97 98 99) nil) ((121) nil)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")))
     (compose-string s 1 4)
     (list (find-composition-internal 0 1 s nil)
	   (find-composition-internal 0 2 s nil)
	   (find-composition-internal 2 nil s nil)
	   (find-composition-internal 4 nil s nil)
	   (find-composition-internal 5 0 s nil))))
 '(nil (1 4 t) (1 4 t) nil (1 4 t)))

(qa
 (with-temp-buffer
   (insert (make-string 32 120))
   (let ((s (copy-sequence "xabcy")) (limit (copy-marker 2)))
     (compose-string s 1 4) (find-composition-internal 0 limit s nil)))
 '(1 4 t))


(princ (qa--done)) (terpri)
