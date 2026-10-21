;;; -*- lexical-binding: t; -*-
;; Issue #249: clearing uses the original Emacs storage byte count, then
;; removes properties and changes nonempty objects to unibyte NUL data;
;; empty strings retain their original byte kind.
(defun qa249-string (kind)
  (cond ((eq kind 'u-ascii) (copy-sequence "ab"))
        ((eq kind 'u-high) (unibyte-string 128 255))
        ((eq kind 'u-empty) (unibyte-string))
        ((eq kind 'm-ascii) (with-temp-buffer (insert "ab") (buffer-string)))
        ((eq kind 'm-two) (string 233 255))
        ((eq kind 'm-three) (copy-sequence "日本"))
        ((eq kind 'm-four) (string #x1f600 #x1f601))
        ((eq kind 'm-five) (string #x200000 #x300000))
        ((eq kind 'm-raw) (string #x3fff80 #x3fffff))
        ((eq kind 'm-surrogate) (string #xd800 #xdfff))
        ((eq kind 'm-high-four) (string #x110000 #x1fffff))
        ((eq kind 'm-mixed) (string ?a 233 #x200000 #x3fff80))
        ((eq kind 'm-empty) (make-string 0 ?a t))))
(defun qa249-snapshot (string)
  (list (length string) (string-bytes string) (multibyte-string-p string)
        (string-to-list string) (object-intervals string)))
(defun qa249-clear (kind styled)
  (let* ((string (qa249-string kind)) (alias string) (other (copy-sequence string))
         (payload (list 'shared)))
    (when (and styled (> (length string) 0))
      (put-text-property 0 1 'face 'bold string)
      (put-text-property 0 (length string) 'qa249 payload string))
    (let ((before (qa249-snapshot string))
          (copy-before (qa249-snapshot other))
          (result (clear-string string)))
      (let ((after (qa249-snapshot string)))
        (garbage-collect)
        (list before result after (eq string alias) (equal after (qa249-snapshot alias))
              (equal copy-before (qa249-snapshot other))
              (progn (clear-string string) (equal after (qa249-snapshot string))))))))

(qa (qa249-clear 'u-ascii nil)
    '((2 2 nil (97 98) nil) nil (2 2 nil (0 0) nil) t t t t))

(qa (qa249-clear 'u-ascii t)
    '((2 2 nil (97 98)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (2 2 nil (0 0) nil) t t t t))

(qa (qa249-clear 'u-high nil)
    '((2 2 nil (128 255) nil) nil (2 2 nil (0 0) nil) t t t t))

(qa (qa249-clear 'u-high t)
    '((2 2 nil (128 255)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (2 2 nil (0 0) nil) t t t t))

(qa (qa249-clear 'u-empty nil)
    '((0 0 nil nil nil) nil (0 0 nil nil nil) t t t t))

(qa (qa249-clear 'u-empty t)
    '((0 0 nil nil nil) nil (0 0 nil nil nil) t t t t))

(qa (qa249-clear 'm-ascii nil)
    '((2 2 t (97 98) nil) nil (2 2 nil (0 0) nil) t t t t))

(qa (qa249-clear 'm-ascii t)
    '((2 2 t (97 98)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (2 2 nil (0 0) nil) t t t t))

(qa (qa249-clear 'm-two nil)
    '((2 4 t (233 255) nil) nil (4 4 nil (0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-two t)
    '((2 4 t (233 255)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (4 4 nil (0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-three nil)
    '((2 6 t (26085 26412) nil) nil (6 6 nil (0 0 0 0 0 0) nil) t t t
      t))

(qa (qa249-clear 'm-three t)
    '((2 6 t (26085 26412)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (6 6 nil (0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-four nil)
    '((2 8 t (128512 128513) nil) nil (8 8 nil (0 0 0 0 0 0 0 0) nil)
      t t t t))

(qa (qa249-clear 'm-four t)
    '((2 8 t (128512 128513)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (8 8 nil (0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-five nil)
    '((2 10 t (2097152 3145728) nil) nil
      (10 10 nil (0 0 0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-five t)
    '((2 10 t (2097152 3145728)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (10 10 nil (0 0 0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-raw nil)
    '((2 4 t (4194176 4194303) nil) nil (4 4 nil (0 0 0 0) nil) t t t
      t))

(qa (qa249-clear 'm-raw t)
    '((2 4 t (4194176 4194303)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (4 4 nil (0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-surrogate nil)
    '((2 6 t (55296 57343) nil) nil (6 6 nil (0 0 0 0 0 0) nil) t t t
      t))

(qa (qa249-clear 'm-surrogate t)
    '((2 6 t (55296 57343)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (6 6 nil (0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-high-four nil)
    '((2 8 t (1114112 2097151) nil) nil
      (8 8 nil (0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-high-four t)
    '((2 8 t (1114112 2097151)
	 ((0 1 (qa249 (shared) face bold)) (1 2 (qa249 (shared)))))
      nil (8 8 nil (0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-mixed nil)
    '((4 10 t (97 233 2097152 4194176) nil) nil
      (10 10 nil (0 0 0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-mixed t)
    '((4 10 t (97 233 2097152 4194176)
	 ((0 1 (qa249 (shared) face bold)) (1 4 (qa249 (shared)))))
      nil (10 10 nil (0 0 0 0 0 0 0 0 0 0) nil) t t t t))

(qa (qa249-clear 'm-empty nil)
    '((0 0 t nil nil) nil (0 0 t nil nil) t t t t))

(qa (qa249-clear 'm-empty t)
    '((0 0 t nil nil) nil (0 0 t nil nil) t t t t))

(qa
 (let ((s (unibyte-string 128 255)))
   (clear-string s)
   (list (length s) (string-bytes s) (string-to-list s)))
 '(2 2 (0 0)))

(qa
 (let ((s (copy-sequence "éあ")))
   (clear-string s)
   (list (length s) (string-bytes s) (multibyte-string-p s)
	 (string-to-list s)))
 '(5 5 nil (0 0 0 0 0)))

(qa
 (let ((s (propertize "abc" 'face nil)))
   (clear-string s)
   (list (object-intervals s) (text-properties-at 0 s)
	 (string-to-list s)))
 '(nil nil (0 0 0)))

(qa
 (let ((s (unibyte-string 128 255)))
   (fillarray s 233) (clear-string s)
   (list (length s) (multibyte-string-p s) (string-to-list s)))
 '(2 nil (0 0)))

(qa
 (let ((s (string 4194176 4194303)))
   (clear-string s) (fillarray s 255)
   (list (length s) (multibyte-string-p s) (string-bytes s)
	 (string-to-list s)))
 '(4 nil 4 (255 255 255 255)))

(qa (condition-case err (clear-string nil) (error err))
    '(wrong-type-argument stringp nil))

(qa (condition-case err (clear-string 42) (error err))
    '(wrong-type-argument stringp 42))

(qa (condition-case err (clear-string 'x) (error err))
    '(wrong-type-argument stringp x))

(qa (condition-case err (clear-string [1 2]) (error err))
    '(wrong-type-argument stringp [1 2]))

(qa (condition-case err (clear-string '(1 2)) (error err))
    '(wrong-type-argument stringp (1 2)))

(qa
 (let ((s (with-temp-buffer (buffer-string))))
   (list (qa249-snapshot s) (clear-string s) (qa249-snapshot s)))
 '((0 0 t nil nil) nil (0 0 t nil nil)))

(qa
 (let ((s (substring (string 233) 0 0)))
   (list (qa249-snapshot s) (clear-string s) (qa249-snapshot s)))
 '((0 0 t nil nil) nil (0 0 t nil nil)))


(princ (qa--done)) (terpri)
