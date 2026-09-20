;; Probe: buffer/primitives.rs remaining branches — sexp/scan
;; motion, undo, search, replacement expansion, markers, columns.
(progn
  ;; ---- sexp motion over strings/comments -------------------------
  (with-temp-buffer
    (insert "(a (b c) \"d (e\" f) ; comment (g)\n(h |i| j)")
    (goto-char (point-min))
    (forward-sexp 1) (prin1 (point))
    (forward-sexp 1) (prin1 (point))
    (forward-sexp 1) (prin1 (point))
    (forward-sexp -2) (prin1 (point))
    (backward-sexp 1) (prin1 (point))
    (goto-char (point-max))
    (backward-sexp 1) (prin1 (point)))
  ;; ---- scan-lists --------------------------------------------------
  (with-temp-buffer
    (insert "(a (b (c) d) e)")
    (goto-char (point-min))
    (prin1 (scan-lists (point) 1 1))
    (prin1 (scan-lists (point) 1 0))
    (prin1 (scan-lists (point) -1 0))
    (condition-case e (scan-lists 2 1 1) (error (prin1 (car e)))))
  ;; ---- scan-sexps -------------------------------------------------
  (with-temp-buffer
    (insert "aa bb (cc dd) \"ee ff\"")
    (goto-char (point-min))
    (prin1 (scan-sexps (point) 2))
    (prin1 (scan-sexps (point) 3))
    (prin1 (scan-sexps (point) 4))
    (prin1 (scan-sexps (point-max) -1)))
  ;; ---- syntax-after -----------------------------------------------
  (with-temp-buffer
    (insert "a (b) ;c\n\"d\"")
    (goto-char (point-min))
    (prin1 (syntax-after (point)))
    (forward-char 1)
    (prin1 (syntax-after (point)))
    (forward-char 4)
    (prin1 (syntax-after (point)))
    (end-of-line)
    (forward-char 1)
    (prin1 (syntax-after (point))))
  ;; ---- search variants --------------------------------------------
  (with-temp-buffer
    (insert "The cat sat on the CAT mat")
    (goto-char (point-min))
    (prin1 (re-search-forward "c\\(a\\)t" nil t))
    (prin1 (match-string 1))
    (goto-char (point-min))
    (let ((case-fold-search nil))
      (prin1 (re-search-forward "CAT" nil t)))
    (goto-char (point-max))
    (prin1 (re-search-backward "cat" nil t))
    (goto-char (point-min))
    (prin1 (word-search-forward "cat" nil t))
    (goto-char (point-min))
    (prin1 (search-forward "cat" nil t))
    (goto-char (point-max))
    (prin1 (search-backward "cat" nil t)))
  ;; ---- replace-match / expand_replacement --------------------------
  (with-temp-buffer
    (insert "hello world")
    (goto-char (point-min))
    (re-search-forward "h\\(ell\\)o")
    (replace-match "H\\1o-\\&")
    (prin1 (buffer-string)))
  (with-temp-buffer
    (insert "abc")
    (goto-char (point-min))
    (re-search-forward "b")
    (replace-match "X" t)
    (prin1 (buffer-string)))
  ;; ---- match-case ---------------------------------------------------
  (with-temp-buffer
    (insert "HELLO hello")
    (goto-char (point-min))
    (let ((case-fold-search t))
      (re-search-forward "hello"))
    (prin1 (match-string 0)))
  ;; ---- move-to-column ------------------------------------------------
  (with-temp-buffer
    (insert "ab\tcd")
    (goto-char (point-min))
    (move-to-column 3)
    (prin1 (point))
    (move-to-column 10)
    (prin1 (point))
    (goto-char (point-min))
    (move-to-column 5 t)
    (prin1 (point)))
  ;; ---- rename-buffer / markers --------------------------------------
  (with-temp-buffer
    (rename-buffer "rn-x")
    (prin1 (buffer-name))
    (rename-buffer (generate-new-buffer-name "rn-x"))
    (condition-case e (rename-buffer "*scratch*") (error (prin1 (car e))))
    (kill-buffer (current-buffer)))
  (let ((m (make-marker)))
    (set-marker m 3)
    (let ((m2 (copy-marker m)))
      (prin1 (marker-position m2))
      (prin1 (eq m m2)))
    (let ((m3 (copy-marker m t)))
      (prin1 (marker-insertion-type m3))))
  ;; ---- insert-before-markers -----------------------------------------
  (with-temp-buffer
    (insert "xy")
    (let ((m (copy-marker 1)))
      (goto-char 1)
      (insert-before-markers "Q")
      (prin1 (marker-position m))
      (prin1 (buffer-string))))
  ;; ---- text-properties-at ---------------------------------------------
  (with-temp-buffer
    (insert "prop")
    (put-text-property 1 3 'face 'bold)
    (prin1 (text-properties-at 1))
    (prin1 (text-properties-at 4)))
  ;; ---- thing-at-point / pop-mark ---------------------------------------
  (with-temp-buffer
    (insert "foo-bar baz")
    (goto-char (point-min))
    (prin1 (thing-at-point 'word))
    (prin1 (thing-at-point 'symbol))
    (prin1 (thing-at-point 'line))
    (push-mark 5)
    (push-mark 8)
    (pop-mark)
    (prin1 (point)))
  ;; ---- undo variants --------------------------------------------------
  (with-current-buffer (get-buffer-create "undo2-buf")
    (insert "abc")
    (let ((buffer-undo-list nil))
      (insert "d")
      (delete-region 1 2)
      (primitive-undo 2 buffer-undo-list)))
  (with-current-buffer (get-buffer-create "undo3-buf")
    (insert "zz")
    (undo 1)
    (prin1 (buffer-string))
    (condition-case e
        (undo-more 1)
      (error (prin1 (car e))))
    (prin1 (buffer-string)))
  ;; ---- forward-comment -------------------------------------------------
  (with-temp-buffer
    (insert "a  ; c1\nb ;c2\n\nc")
    (goto-char (point-min))
    (forward-comment 1) (prin1 (point))
    (forward-comment 1) (prin1 (point))
    (forward-comment -1) (prin1 (point))
    (goto-char (point-max))
    (forward-comment -1) (prin1 (point)))
  ;; ---- regexp-opt --------------------------------------------------------
  (prin1 (regexp-opt '("foo" "bar")))
  (prin1 (regexp-opt '("cat" "car") t))
  (prin1 (regexp-opt nil))
  ;; ---- forward-char boundaries -------------------------------------------
  (with-temp-buffer
    (insert "ab")
    (goto-char (point-min))
    (condition-case e (forward-char -5) (error (prin1 (car e))))
    (prin1 (point))
    (goto-char (point-max))
    (condition-case e (forward-char 10) (error (prin1 (car e))))
    (prin1 (point))
    (forward-char 0)
    (prin1 (point)))
  (prin1 'done))
