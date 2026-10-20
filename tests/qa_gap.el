;;; -*- lexical-binding: t; -*-
;; Issue #114: allocation is byte-based; growth leaves spare headroom.
(defvar qa114-case)
(defvar qa114-size)
(dolist (qa114-case '((0 20) (1 19) (19 1) (20 0) (21 2000) (100 2000)
                     (1999 2000) (2000 2000) (2001 2000) (5000 2000) (10000 2000)))
  (qa (with-temp-buffer
        (insert (make-string (car qa114-case) ?a))
        (list (buffer-size) (gap-position) (gap-size)))
      (list (car qa114-case) (1+ (car qa114-case)) (cadr qa114-case))))
(with-temp-buffer
  (qa (list (gap-position) (gap-size)) '(1 20))
  (dotimes (_ 20) (insert "a"))
  (qa (list (gap-position) (gap-size)) '(21 0))
  (insert "b")
  (qa (list (gap-position) (gap-size)) '(22 2000))
  (insert "c")
  (qa (list (gap-position) (gap-size)) '(23 1999)))
(qa (with-temp-buffer
      (insert (make-string 100 ?a)) (delete-region 50 90)
      (list (buffer-size) (gap-position) (gap-size))) '(60 50 2040))
(qa (with-temp-buffer
      (insert (make-string 5000 ?a)) (erase-buffer)
      (list (buffer-size) (gap-position) (gap-size))) '(0 1 7000))
(qa (with-temp-buffer
      (insert "abcdef") (goto-char 3) (insert "X")
      (list (buffer-string) (gap-position) (gap-size))) '("abXcdef" 4 13))
(with-temp-buffer
  (insert "abcdef") (goto-char 3) (insert "X")
  (goto-char 1) (skip-chars-forward "a-zA-Z")
  (qa (list (gap-position) (gap-size)) '(4 13))
  (goto-char 1) (skip-syntax-forward "w")
  (qa (list (gap-position) (gap-size)) '(4 13))
  (scan-sexps 1 1)
  (qa (list (gap-position) (gap-size)) '(4 13))
  (goto-char 1) (search-forward "ef")
  (qa (list (gap-position) (gap-size)) '(4 13))
  (buffer-string) (buffer-substring 1 4)
  (qa (list (gap-position) (gap-size)) '(4 13))
  (set-text-properties 1 (point-max) '(face bold))
  (qa (list (gap-position) (gap-size)) '(4 13))
  (goto-char (point-max)) (insert "")
  (qa (list (gap-position) (gap-size)) '(4 13)))
(dolist (qa114-case (list (list "é" 1 18) (list "日本語" 3 11) (list "😀" 1 16)
                         (list (unibyte-string 128 255) 2 16)
                         (list (string #xD800) 1 17)
                         (list (string #x110001 #x200001) 2 11)))
  (qa (with-temp-buffer
        (insert (car qa114-case))
        (list (buffer-size) (gap-position) (gap-size)))
      (list (cadr qa114-case) (1+ (cadr qa114-case)) (caddr qa114-case))))
(qa (with-temp-buffer
      (insert "日本語") (delete-region 2 3)
      (list (buffer-string) (gap-position) (gap-size))) '("日語" 2 14))
(qa (with-temp-buffer
      (set-buffer-multibyte nil) (insert (unibyte-string 128 255))
      (list (buffer-size) (gap-position) (gap-size))) '(2 3 18))
(qa (with-temp-buffer
      (set-buffer-multibyte nil) (insert (unibyte-string 128 255))
      (set-buffer-multibyte t)
      (list (buffer-size) (gap-position) (gap-size))) '(2 3 16))
(dolist (qa114-case '((100000 3592) (200000 5155)))
  (qa (with-temp-buffer
        (insert (make-string (car qa114-case) ?a))
        (dotimes (_ 2000) (insert "b"))
        (insert "c")
        (list (buffer-size) (gap-position) (gap-size)))
      (list (+ (car qa114-case) 2001) (+ (car qa114-case) 2002) (cadr qa114-case))))
(princ (qa--done)) (terpri)
