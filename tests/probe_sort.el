;; Sort cluster semantics probe
(defmacro sh (tag &rest body)
  `(let ((b (get-buffer-create " *t*")))
     (with-current-buffer b
       (erase-buffer)
       (insert ,@body)
       (goto-char (point-min))
       (condition-case e (progn ,@body) (error (princ (format "%s ERR %S\n" ,tag e)))))))
(princ "==sort-lines==\n")
(with-temp-buffer
  (insert "banana\napple\ncherry\n")
  (sort-lines nil (point-min) (point-max))
  (princ (buffer-string)))
(with-temp-buffer
  (insert "B\na\nC\nb\n")
  (let ((sort-fold-case t)) (sort-lines nil (point-min) (point-max)))
  (princ (buffer-string)))
(with-temp-buffer
  (insert "b\na\nC\n")
  (sort-lines t (point-min) (point-max))
  (princ (buffer-string)))
;; trailing partial line
(with-temp-buffer
  (insert "z\na\nx")
  (sort-lines nil (point-min) (point-max))
  (princ (buffer-string)) (princ "|\n"))
(princ "==sort-fields==\n")
(with-temp-buffer
  (insert "one 3\nthree 1\ntwo 2\n")
  (sort-fields 2 (point-min) (point-max))
  (princ (buffer-string)))
(with-temp-buffer
  (insert "3 one\n1 three\n2 two\n")
  (sort-fields -1 (point-min) (point-max))
  (princ (buffer-string)))
;; too few fields error
(with-temp-buffer
  (insert "aa bb\ncc\n")
  (condition-case e (sort-fields 2 (point-min) (point-max))
    (error (princ (format "ERR %S\n" e)))))
(princ "==sort-numeric-fields==\n")
(with-temp-buffer
  (insert "one 30\nthree 1\ntwo 2\n")
  (sort-numeric-fields 2 (point-min) (point-max))
  (princ (buffer-string)))
;; hex + octal + blank line + negative num
(with-temp-buffer
  (insert "a 0x10\nb 010\nc -3\nd\n")
  (condition-case e (progn (sort-numeric-fields 2 (point-min) (point-max))
                           (princ (buffer-string)))
    (error (princ (format "ERR %S\n" e)))))
(with-temp-buffer
  (insert "a x10\nb x2\n")
  (sort-numeric-fields 2 (point-min) (point-max))
  (princ (buffer-string)))
(princ "==sort-regexp-fields==\n")
;; key = \1 group of record regexp
(with-temp-buffer
  (insert "k3v\nk1v\nk2v\n")
  (sort-regexp-fields nil "k\\([0-9]\\)v" "\\1" (point-min) (point-max))
  (princ (buffer-string)))
;; key regexp searched within record
(with-temp-buffer
  (insert "axbxc\nawbwc\navbvc\n")
  (sort-regexp-fields nil "^.*$" "w\\|v" (point-min) (point-max))
  (princ (buffer-string)))
;; records with gaps (non-record text stays in place)
(with-temp-buffer
  (insert "Hdr\nrec2\nmid\nrec1\n")
  (sort-regexp-fields nil "^rec.*$" "\\&" (point-min) (point-max))
  (princ (buffer-string)))
;; record not matching key regexp -> ignored
(with-temp-buffer
  (insert "z9\na1\nq2\n")
  (sort-regexp-fields nil "^.*$" "[aq]" (point-min) (point-max))
  (princ (buffer-string)))
(princ "==sort-columns==\n")
(with-temp-buffer
  (insert "za1\nyb2\nxc3\n")
  (goto-char (point-min)) (forward-char 1)
  (let ((p (point)))
    (goto-char (point-max)) (backward-char 1)
    (sort-columns nil p (point)))
  (princ (buffer-string)))
;; tab rejection
(with-temp-buffer
  (insert "a\tb\n")
  (condition-case e (sort-columns nil (point-min) (point-max))
    (error (princ (format "ERR %s\n" (car e))))))
(princ "==reverse-region==\n")
(with-temp-buffer
  (insert "one\ntwo\nthree\n")
  (reverse-region (point-min) (point-max))
  (princ (buffer-string)))
;; partial-line semantics: beg mid-line -> first line reversed starts AFTER beg
(with-temp-buffer
  (insert "hd1\nhd2\na\nb\nc\ntl\n")
  (reverse-region 4 26)
  (princ (buffer-string)) (princ "|\n"))
;; no full lines -> user-error
(with-temp-buffer
  (insert "abc")
  (condition-case e (reverse-region (point-min) (point-max))
    (error (princ (format "ERR %S\n" e)))))
(princ "==delete-duplicate-lines==\n")
(with-temp-buffer
  (insert "a\nb\na\nc\nb\n")
  (princ (format "n=%d " (delete-duplicate-lines (point-min) (point-max))))
  (princ (buffer-string)))
(with-temp-buffer
  (insert "a\nb\na\nc\nb\n")
  (princ (format "n=%d " (delete-duplicate-lines (point-min) (point-max) t)))
  (princ (buffer-string)))
(with-temp-buffer
  (insert "a\na\nb\na\na\nc\n")
  (princ (format "n=%d " (delete-duplicate-lines (point-min) (point-max) nil t)))
  (princ (buffer-string)))
(with-temp-buffer
  (insert "a\n\n\nb\n\nc\n")
  (princ (format "n=%d " (delete-duplicate-lines (point-min) (point-max) nil nil t)))
  (princ (buffer-string)))
(princ "==compare-buffer-substrings==\n")
(with-temp-buffer
  (insert "abcabd")
  (princ (format "%d %d %d %d %d\n"
                 (compare-buffer-substrings nil 1 4 nil 4 7)
                 (compare-buffer-substrings nil 1 4 nil 1 4)
                 (compare-buffer-substrings nil 4 7 nil 1 4)
                 (compare-buffer-substrings nil 1 2 nil 1 4)
                 (compare-buffer-substrings nil 1 4 nil 1 2))))
(princ "==sort-subr direct==\n")
;; sort records = pairs of lines; key = second line of pair
(with-temp-buffer
  (insert "hdrA\nk2\nhdrB\nk1\n")
  (goto-char (point-min))
  (sort-subr nil
             (lambda () (forward-line 2))
             (lambda () (forward-line 2))
             (lambda () (forward-line 1) nil)
             (lambda () (end-of-line) nil))
  (princ (buffer-string)))
;; numeric keys returned by startkeyfun
(with-temp-buffer
  (insert "30\n1\n2\n")
  (goto-char (point-min))
  (sort-subr nil 'forward-line 'end-of-line
             (lambda () (string-to-number
                         (buffer-substring (point) (line-end-position)))))
  (princ (buffer-string)))
;; predicate arg: keys are cons cells (beg . end)
(with-temp-buffer
  (insert "a\nbb\nc\n")
  (goto-char (point-min))
  (sort-subr nil 'forward-line 'end-of-line nil nil
             (lambda (x y) (> (- (cdr x) (car x)) (- (cdr y) (car y)))))
  (princ (buffer-string)))
;; stability: equal keys keep original order
(with-temp-buffer
  (insert "a2\na1\nb1\na3\n")
  (sort-fields 1 (point-min) (point-max))
  (princ (buffer-string)))
(princ "==sort-paragraphs==\n")
(with-temp-buffer
  (insert "paraB1\nparaB2\n\nparaA1\nparaA2\n")
  (sort-paragraphs nil (point-min) (point-max))
  (princ (buffer-string)))
(princ "==sort-pages==\n")
(with-temp-buffer
  (insert "zc\n\fyb\nxa\n\fw\n")
  (sort-pages nil (point-min) (point-max))
  (princ (buffer-string)) (princ "|\n"))
(princ "==return values==\n")
(with-temp-buffer
  (insert "b\na\n")
  (princ (format "sl=%S " (sort-lines nil (point-min) (point-max))))
  (princ (format "sf=%S " (progn (erase-buffer) (insert "b 1\na 2\n")
                                 (sort-fields 1 (point-min) (point-max)))))
  (princ (format "ddl=%S\n" (progn (erase-buffer) (insert "x\n")
                                   (delete-duplicate-lines (point-min) (point-max))))))
