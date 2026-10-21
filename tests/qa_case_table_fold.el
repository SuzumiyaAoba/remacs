;;; -*- lexical-binding: t; -*-
;; Issue #259: comparison and search use the installed buffer case table.
(defun qa259-profile (upper lower pattern text fold)
 (with-temp-buffer
  (let* ((table (copy-case-table (standard-case-table)))
         (syntax (standard-syntax-table))
         (old-upper (aref syntax upper)) (old-lower (aref syntax lower))
         (case-fold-search fold))
   (unwind-protect
    (progn
     (set-case-syntax-pair upper lower table) (set-case-table table)
     (insert text) (goto-char 1)
     (let* ((needle (char-to-string upper))
            (hit (string-match pattern text))
            (matched (and hit (string-to-list (match-string 0 text)))))
      (list (char-equal upper lower)
       (compare-strings needle nil nil (char-to-string lower) nil nil t)
       hit matched (string-match-p pattern text)
       (progn (goto-char 1) (search-forward needle nil t))
       (progn (goto-char 1) (re-search-forward pattern nil t))
       (progn (goto-char (point-max)) (re-search-backward pattern nil t))
       (progn (goto-char 1) (looking-at pattern))
       (posix-string-match pattern text))))
    (aset syntax upper old-upper) (aset syntax lower old-lower)))))
(qa
(qa259-profile 88 121 "X" "y" t)
'(t t 0 (121) 0 2 2 1 t 0)
)

(qa
(qa259-profile 88 121 "X" "y" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "[X]" "y" t)
'(t t 0 (121) 0 2 2 1 t 0)
)

(qa
(qa259-profile 88 121 "[X]" "y" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "[X-X]" "y" t)
'(t t 0 (121) 0 2 2 1 t 0)
)

(qa
(qa259-profile 88 121 "[X-X]" "y" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "[^X]" "y" t)
'(t t nil nil nil 2 nil nil nil nil)
)

(qa
(qa259-profile 88 121 "[^X]" "y" nil)
'(nil t 0 (121) 0 nil 2 1 t 0)
)

(qa
(qa259-profile 88 121 "X+" "yyy" t)
'(t t 0 (121 121 121) 0 2 4 3 t 0)
)

(qa
(qa259-profile 88 121 "X+" "yyy" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "\\(X\\)\\1" "yy" t)
'(t t 0 (121 121) 0 2 3 1 t 0)
)

(qa
(qa259-profile 88 121 "\\(X\\)\\1" "yy" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "\\bX\\b" "y" t)
'(t t 0 (121) 0 2 2 1 t 0)
)

(qa
(qa259-profile 88 121 "\\bX\\b" "y" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "[[:upper:]]" "y" t)
'(t t 0 (121) 0 2 2 1 t 0)
)

(qa
(qa259-profile 88 121 "[[:upper:]]" "y" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 88 121 "X" "x" t)
'(t t 0 (120) 0 2 2 1 t 0)
)

(qa
(qa259-profile 88 121 "X" "x" nil)
'(nil t nil nil nil nil nil nil nil nil)
)

(qa
(qa259-profile 65 122 (char-to-string 65) (char-to-string 122) t)
'(t t 0 (122) 0 2 2 1 t 0)
)

(qa
(qa259-profile 49 57 (char-to-string 49) (char-to-string 57) t)
'(t t 0 (57) 0 2 2 1 t 0)
)

(qa
(qa259-profile 1114112 1310720 (char-to-string 1114112)
	       (char-to-string 1310720) t)
'(t t 0 (1310720) 0 2 2 1 t 0)
)

(qa
(list (compare-strings "ß" nil nil "S" nil nil t)
      (compare-strings "ﬀ" nil nil "F" nil nil t)
      (compare-strings "ß" nil nil "ẞ" nil nil t))
'(1 1 t)
)

(qa
(with-temp-buffer
  (set-buffer-multibyte nil)
  (let ((case-fold-search t))
    (list (char-equal 233 201) (char-equal 233 4194281)
	  (char-equal 1114112 1114113))))
'(nil t nil)
)

(qa
(let ((case-fold-search t))
  (list (char-equal 1114112 1114113) (char-equal 55296 55297)
	(char-equal 4194176 4194177)))
'(nil nil nil)
)

(qa
(let
    ((first (generate-new-buffer " *qa259-first*"))
     (second (generate-new-buffer " *qa259-second*")))
  (unwind-protect
      (progn
	(with-current-buffer first
	  (let ((table (copy-case-table (standard-case-table))))
	    (set-case-syntax-pair 88 121 table) (set-case-table table)))
	(with-current-buffer second
	  (let ((table (copy-case-table (standard-case-table))))
	    (set-case-syntax-pair 88 122 table) (set-case-table table)))
	(list
	 (with-current-buffer first
	   (let ((case-fold-search t))
	     (list (string-match "X" "y") (string-match "X" "z"))))
	 (with-current-buffer second
	   (let ((case-fold-search t))
	     (list (string-match "X" "y") (string-match "X" "z"))))
	 (with-current-buffer first
	   (let ((case-fold-search t)) (string-match "X" "y")))))
    (kill-buffer first) (kill-buffer second)))
'((0 nil) (nil 0) 0)
)

(qa
(with-temp-buffer
  (let
      ((table (copy-case-table (standard-case-table)))
       (case-fold-search t))
    (set-case-syntax-pair 88 121 table) (set-case-table table)
    (let ((before (string-match "X" "z")))
      (set-case-syntax-pair 88 122 table) (set-case-table table)
      (list before (string-match "X" "z") (char-equal 88 121)
	    (char-equal 88 122)))))
'(nil 0 nil t)
)

(qa
(with-temp-buffer
  (let
      ((table (copy-case-table (standard-case-table)))
       (case-fold-search nil))
    (set-case-syntax-pair 88 121 table) (set-case-table table)
    (list (compare-strings "aXb" 1 2 "ayc" 1 2 t)
	  (compare-strings "aXb" -2 -1 "ayc" -2 -1 t)
	  (compare-strings "aXb" nil nil "ayc" nil nil t)
	  (compare-strings "aXb" 1 2 "ayc" 1 2 nil))))
'(t t -3 -1)
)

(qa
(with-temp-buffer
  (let
      ((table (copy-case-table (standard-case-table)))
       (case-fold-search t))
    (set-case-table table) (set-case-syntax-pair 88 121 table)
    (let ((before (char-table-p (char-table-extra-slot table 1))))
      (list before (char-equal 88 121)
	    (char-table-p (char-table-extra-slot table 1))))))
'(nil t nil)
)

(qa
(with-temp-buffer
  (let
      ((table (copy-case-table (standard-case-table)))
       (case-fold-search t))
    (set-case-table table) (set-case-syntax-pair 88 121 table)
    (let ((before (char-table-p (char-table-extra-slot table 1))))
      (list before (compare-strings "X" nil nil "y" nil nil t)
	    (char-table-p (char-table-extra-slot table 1))))))
'(nil t t)
)

(qa
(with-temp-buffer
  (let
      ((table (copy-case-table (standard-case-table)))
       (case-fold-search t))
    (set-case-table table) (set-case-syntax-pair 88 121 table)
    (let ((before (char-table-p (char-table-extra-slot table 1))))
      (list before (string-match "X" "y")
	    (char-table-p (char-table-extra-slot table 1))))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "ß") (goto-char 1)
    (list (string-match "S" "ß") (re-search-forward "S" nil t)
	  (progn (goto-char 1) (looking-at "S")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "ß") (goto-char 1)
    (list (string-match "S" "ß") (re-search-forward "S" nil t)
	  (progn (goto-char 1) (looking-at "S")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "ß") (goto-char 1)
    (list (string-match "[S]" "ß") (re-search-forward "[S]" nil t)
	  (progn (goto-char 1) (looking-at "[S]")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "ß") (goto-char 1)
    (list (string-match "[S]" "ß") (re-search-forward "[S]" nil t)
	  (progn (goto-char 1) (looking-at "[S]")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "ﬀ") (goto-char 1)
    (list (string-match "[F]" "ﬀ") (re-search-forward "[F]" nil t)
	  (progn (goto-char 1) (looking-at "[F]")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "ﬀ") (goto-char 1)
    (list (string-match "[F]" "ﬀ") (re-search-forward "[F]" nil t)
	  (progn (goto-char 1) (looking-at "[F]")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "ς") (goto-char 1)
    (list (string-match "Σ" "ς") (re-search-forward "Σ" nil t)
	  (progn (goto-char 1) (looking-at "Σ")))))
'(0 2 t)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "ς") (goto-char 1)
    (list (string-match "Σ" "ς") (re-search-forward "Σ" nil t)
	  (progn (goto-char 1) (looking-at "Σ")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "ς") (goto-char 1)
    (list (string-match "[Σ]" "ς") (re-search-forward "[Σ]" nil t)
	  (progn (goto-char 1) (looking-at "[Σ]")))))
'(0 2 t)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "ς") (goto-char 1)
    (list (string-match "[Σ]" "ς") (re-search-forward "[Σ]" nil t)
	  (progn (goto-char 1) (looking-at "[Σ]")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "ß") (goto-char 1)
    (list (string-match "[[:lower:]]" "ß")
	  (re-search-forward "[[:lower:]]" nil t)
	  (progn (goto-char 1) (looking-at "[[:lower:]]")))))
'(0 2 t)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "ß") (goto-char 1)
    (list (string-match "[[:lower:]]" "ß")
	  (re-search-forward "[[:lower:]]" nil t)
	  (progn (goto-char 1) (looking-at "[[:lower:]]")))))
'(0 2 t)
)

(qa
(with-temp-buffer
  (let ((case-fold-search t))
    (insert "i") (goto-char 1)
    (list (string-match "İ" "i") (re-search-forward "İ" nil t)
	  (progn (goto-char 1) (looking-at "İ")))))
'(nil nil nil)
)

(qa
(with-temp-buffer
  (let ((case-fold-search nil))
    (insert "i") (goto-char 1)
    (list (string-match "İ" "i") (re-search-forward "İ" nil t)
	  (progn (goto-char 1) (looking-at "İ")))))
'(nil nil nil)
)

(qa
(let ((left "x") (right 120))
  (condition-case err (char-equal left right)
    (error
     (list (car err) (cadr err) (eq (nth 2 err) left)
	   (eq (nth 2 err) right)))))
'(wrong-type-argument characterp t nil)
)

(qa
(let ((left 120) (right "x"))
  (condition-case err (char-equal left right)
    (error
     (list (car err) (cadr err) (eq (nth 2 err) left)
	   (eq (nth 2 err) right)))))
'(wrong-type-argument characterp nil t)
)

(qa
(let ((left (expt 2 100)) (right 120))
  (condition-case err (char-equal left right)
    (error
     (list (car err) (cadr err) (eq (nth 2 err) left)
	   (eq (nth 2 err) right)))))
'(wrong-type-argument characterp t nil)
)

(qa
(let ((left 120) (right (point-marker)))
  (condition-case err (char-equal left right)
    (error
     (list (car err) (cadr err) (eq (nth 2 err) left)
	   (eq (nth 2 err) right)))))
'(wrong-type-argument characterp nil t)
)

(qa
(with-temp-buffer
  (set-buffer-multibyte nil)
  (list (compare-strings (string 233) nil nil (string 201) nil nil t)
	(compare-strings (unibyte-string 233) nil nil
			 (unibyte-string 201) nil nil t)
	(compare-strings "ß" nil nil "ẞ" nil nil t)))
'(1 1 -1)
)

(qa
(with-temp-buffer
  (let*
      ((table (copy-case-table (standard-case-table)))
       (syntax (standard-syntax-table)) (old (aref syntax 256))
       (old-a (aref syntax 97)))
    (unwind-protect
	(progn
	  (set-case-syntax-pair 256 97 table) (set-case-table table)
	  (set-buffer-multibyte nil)
	  (list (compare-strings (string 256) nil nil "a" nil nil t)
		(compare-strings "a" nil nil (string 256) nil nil t)))
      (aset syntax 256 old) (aset syntax 97 old-a))))
'(1 -1)
)

(qa
(with-temp-buffer
  (let*
      ((table (copy-case-table (standard-case-table)))
       (syntax (standard-syntax-table)) (old (aref syntax 4194249))
       (old-lower (aref syntax 4194281)))
    (unwind-protect
	(progn
	  (set-case-syntax-pair 4194249 4194281 table)
	  (set-case-table table) (set-buffer-multibyte nil)
	  (list
	   (compare-strings (string 233) nil nil (string 201) nil nil
			    t)
	   (compare-strings (unibyte-string 233) nil nil
			    (unibyte-string 201) nil nil t)))
      (aset syntax 4194249 old) (aset syntax 4194281 old-lower))))
'(t t)
)

(qa
(with-temp-buffer
  (let ((table (copy-case-table (standard-case-table))))
    (set-case-table table) (set-case-syntax-pair 88 121 table)
    (list (compare-strings "X" nil nil "X" nil nil t)
	  (char-table-p (char-table-extra-slot table 1)))))
'(t nil)
)

(qa
(qa259-profile 1114112 1310720
	       (concat "[" (char-to-string 1114112) "]")
	       (char-to-string 1310720) t)
'(t t 0 (1310720) 0 2 2 1 t 0)
)

(qa
(qa259-profile 1114112 1310720
	       (concat "\\(" (char-to-string 1114112) "\\)\\1")
	       (make-string 2 1310720) t)
'(t t 0 (1310720 1310720) 0 2 3 1 t 0)
)


;; Issue #260: retain distinct non-Unicode codes while honoring real case pairs.
(qa
(let ((case-fold-search t))
  (list (char-equal 1114112 1114113) (char-equal 55296 55297)
	(char-equal 4194176 4194303)))
'(nil nil nil)
)

(qa
(let ((case-fold-search nil))
  (list (char-equal 1114112 1114113) (char-equal 55296 55297)
	(char-equal 4194176 4194303)))
'(nil nil nil)
)

(qa
(let ((case-fold-search t))
  (mapcar (lambda (code) (char-equal code code))
	  '(0 65 233 55296 57343 1114112 1310720 4194176 4194303)))
'(t t t t t t t t t)
)

(qa
(let ((case-fold-search nil))
  (mapcar (lambda (code) (char-equal code code))
	  '(0 65 233 55296 57343 1114112 1310720 4194176 4194303)))
'(t t t t t t t t t)
)

(qa
(let ((case-fold-search t))
  (mapcar
   (lambda (pair)
     (list (char-equal (car pair) (cadr pair))
	   (char-equal (cadr pair) (car pair))))
   '((55296 55297) (56319 56320) (57343 1114112) (1114112 1114113)
     (1114112 1310720) (4194176 4194303))))
'((nil nil) (nil nil) (nil nil) (nil nil) (nil nil) (nil nil))
)

(qa
(let ((case-fold-search nil))
  (mapcar
   (lambda (pair)
     (list (char-equal (car pair) (cadr pair))
	   (char-equal (cadr pair) (car pair))))
   '((55296 55297) (56319 56320) (57343 1114112) (1114112 1114113)
     (1114112 1310720) (4194176 4194303))))
'((nil nil) (nil nil) (nil nil) (nil nil) (nil nil) (nil nil))
)

(qa
(with-temp-buffer
  (let*
      ((table (copy-case-table (standard-case-table)))
       (syntax (standard-syntax-table))
       (old-upper (aref syntax 55296)) (old-lower (aref syntax 55297)))
    (unwind-protect
	(progn
	  (set-case-syntax-pair 55296 55297 table)
	  (set-case-table table)
	  (list (let ((case-fold-search t)) (char-equal 55296 55297))
		(let ((case-fold-search nil))
		  (char-equal 55296 55297))))
      (aset syntax 55296 old-upper) (aset syntax 55297 old-lower))))
'(t nil)
)

(qa
(with-temp-buffer
  (let*
      ((table (copy-case-table (standard-case-table)))
       (syntax (standard-syntax-table))
       (old-upper (aref syntax 1114112))
       (old-lower (aref syntax 1310720)))
    (unwind-protect
	(progn
	  (set-case-syntax-pair 1114112 1310720 table)
	  (set-case-table table)
	  (list
	   (let ((case-fold-search t)) (char-equal 1114112 1310720))
	   (let ((case-fold-search nil)) (char-equal 1114112 1310720))))
      (aset syntax 1114112 old-upper) (aset syntax 1310720 old-lower))))
'(t nil)
)

(qa
(with-temp-buffer
  (let*
      ((table (copy-case-table (standard-case-table)))
       (syntax (standard-syntax-table))
       (old-upper (aref syntax 4194176))
       (old-lower (aref syntax 4194303)))
    (unwind-protect
	(progn
	  (set-case-syntax-pair 4194176 4194303 table)
	  (set-case-table table)
	  (list
	   (let ((case-fold-search t)) (char-equal 4194176 4194303))
	   (let ((case-fold-search nil)) (char-equal 4194176 4194303))))
      (aset syntax 4194176 old-upper) (aset syntax 4194303 old-lower))))
'(t nil)
)

(qa
(with-temp-buffer
  (set-buffer-multibyte t)
  (list (let ((case-fold-search t)) (char-equal 233 4194281))
	(let ((case-fold-search nil)) (char-equal 233 4194281))))
'(nil nil)
)

(qa
(with-temp-buffer
  (set-buffer-multibyte nil)
  (list (let ((case-fold-search t)) (char-equal 233 4194281))
	(let ((case-fold-search nil)) (char-equal 233 4194281))))
'(t nil)
)


(princ (qa--done)) (terpri)
