;; Match data / regexp utilities / symbol-at-point — GNU 31.1 pins.
;;
;; GNU truths pinned:
;;   - match-data-integers / string-match-positions / word-at-point are
;;     NOT fns in 31.1.
;;   - regexp-opt-depth/charset/group are fns; -word-count/-string are not.
;;   - posix-string-match returns 0-based index of match start.
;;   - symbol-at-point returns a SYMBOL; word-at-point is not in 31.1.
;;   - regexp-quote escapes ., *, [ but NOT ] alone.
;;   - match-data returns a list of integer positions when set via
;;     set-match-data with ints; with marker buffer searches, markers.
;;   - save-match-data preserves the caller's match-data.

;; == fn surface ==
(qa (fboundp 'match-data-integers) nil)
(qa (fboundp 'string-match-positions) nil)
(qa (fboundp 'regexp-opt-depth) t)
(qa (fboundp 'regexp-opt-charset) t)
(qa (fboundp 'regexp-opt-word-count) nil)
(qa (fboundp 'regexp-opt-string) nil)
(qa (fboundp 'regexp-opt-string-overlaps) nil)
(qa (fboundp 'regexp-opt-group) t)
(qa (fboundp 'regexp-opt-nesting-depth) nil)
(qa (fboundp 'regexp-opt-default-syntax-table) nil)
(qa (fboundp 'symbol-at-point) t)
(qa (fboundp 'word-at-point) nil)

;; == posix-string-match ==
(qa (posix-string-match "\\([a-z]\\)" "xy q ab") 0)
(qa (posix-string-match "ZZZ" "xy q ab") nil)
(qa (posix-string-match "q" "xy q ab") 3)

;; == symbol-at-point / word-at-point ==
(with-temp-buffer
  (insert "hello world")
  (goto-char 2)
  (qa (symbol-at-point) 'hello))
;; word-at-point not in 31.1

;; == regexp-quote ==
(qa (regexp-quote "a.b*c") "a\\.b\\*c")
;; ] alone is not escaped
(qa (regexp-quote "foo[bar]") "foo\\[bar]")
(qa (regexp-quote "^anchor$") "\\^anchor\\$")
(qa (regexp-quote "plain") "plain")
(qa (regexp-quote "") "")

;; == match-data / set-match-data ==
(with-temp-buffer
  (insert "ab-cd")
  (goto-char 1)
  (re-search-forward "\\([a-z]\\)-\\([a-z]\\)")
  ;; match-string returns matched substrings
  (qa (match-string 0) "b-c")
  (qa (match-string 1) "b")
  (qa (match-string 2) "c")
  ;; whole + 2 groups → 6 entries (markers in GNU)
  (qa (length (match-data)) 6)
  ;; GNU returns real markers in buffer searches
  (let ((md (match-data)))
    ;; remacs: match-data returns integer positions, not markers
    (qa (markerp (car md)) t)
    (qa (marker-position (car md)) 2)
    (qa (marker-position (cadr md)) 5))
  ;; set-match-data '(nil nil) clears the match entirely
  (set-match-data '(nil nil))
  ;; remacs: returns (nil nil) instead of nil
  (qa (match-data) nil))

;; == match-beginning/-end / match-string-no-properties ==
(with-temp-buffer
  (insert "abc123def")
  (goto-char 1)
  (re-search-forward "[0-9]+")
  (qa (match-beginning 0) 4)
  (qa (match-end 0) 7)
  (qa (match-string 0) "123")
  (qa (match-string-no-properties 0) "123")
  (qa (match-beginning 1) nil)
  (qa (match-string 1) nil))

;; == save-match-data preserves across nested searches ==
(with-temp-buffer
  (insert "foo")
  (goto-char 1)
  (re-search-forward "o")
  (let ((md (match-data)))
    (save-match-data
      (ignore-errors (re-search-forward "o")))
    (qa (equal md (match-data)) t)))

;; GH #134: only complete start/end pairs survive set-match-data.
(qa (progn (string-match "b" "abc") (set-match-data '(1)) (match-data)) nil)
(qa (progn (string-match "b" "abc") (set-match-data '(1 2 3)) (match-data)) '(1 2))
(qa (progn (string-match "b" "abc") (set-match-data '(ignored)) (match-data)) nil)
(qa (progn (string-match "b" "abc") (set-match-data '(1 2 ignored)) (match-data)) '(1 2))
(qa (progn (set-match-data '(1 2 3 4 5)) (match-data)) '(1 2 3 4))
(qa (let ((data (list 1 2 3)))
      (set-match-data data) data) '(1 2 3))
(qa (with-temp-buffer
      (insert "abc")
      (let ((marker (copy-marker 2)))
        (set-match-data (list 1 2 marker) t)
        (eq (marker-buffer marker) (current-buffer)))) t)

;; Issue #275: reuse cons cells and reseat previous markers.
(defun qa275-normalize (items source)
 (mapcar (lambda (x) (cond ((markerp x) (list 'marker (marker-position x) (eq (marker-buffer x) source)))
                         ((bufferp x) (list 'buffer (eq x source))) (t x))) items))
(qa
(let ((reuse nil))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(nil (1 2) nil)
)

(qa
(let ((reuse 42))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(nil (1 2) 42)
)

(qa
(let ((reuse 'old))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(nil (1 2) old)
)

(qa
(let ((reuse (list 'old)))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(t (1 2) (1 2))
)

(qa
(let ((reuse (list 'old 'old)))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(t (1 2) (1 2))
)

(qa
(let ((reuse (make-list 6 'old)))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(t (1 2 nil nil nil nil) (1 2 nil nil nil nil))
)

(qa
(let ((reuse (cons 'old 'tail)))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(t (1 2) (1 2))
)

(qa
(let ((reuse (cons 'old (cons 'old (cons 'old 'tail)))))
  (string-match "b" "abc")
  (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))
'(t (1 2 nil . tail) (1 2 nil . tail))
)

(qa
(with-temp-buffer
  (insert "ab") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer)) (reuse (match-data))
       (start (car reuse)) (end (cadr reuse)))
    (re-search-forward "b")
    (let ((out (match-data nil reuse nil)))
      (list (eq out reuse) (qa275-normalize out source)
	    (list (marker-position start) (marker-position end)
		  (eq (marker-buffer start) source)
		  (eq (marker-buffer end) source))
	    (eq (car out) start)))))
'(t ((marker 2 t) (marker 3 t)) (1 2 t t) nil)
)

(qa
(with-temp-buffer
  (insert "ab") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer)) (reuse (match-data))
       (start (car reuse)) (end (cadr reuse)))
    (re-search-forward "b")
    (let ((out (match-data nil reuse t)))
      (list (eq out reuse) (qa275-normalize out source)
	    (list (marker-position start) (marker-position end)
		  (eq (marker-buffer start) source)
		  (eq (marker-buffer end) source))
	    (eq (car out) start)))))
'(t ((marker 2 t) (marker 3 t)) (nil nil nil nil) nil)
)

(qa
(with-temp-buffer
  (insert "ab") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer)) (reuse (match-data))
       (start (car reuse)) (end (cadr reuse)))
    (re-search-forward "b")
    (let ((out (match-data t reuse nil)))
      (list (eq out reuse) (qa275-normalize out source)
	    (list (marker-position start) (marker-position end)
		  (eq (marker-buffer start) source)
		  (eq (marker-buffer end) source))
	    (eq (car out) start)))))
'(t (2 3 (buffer t)) (1 2 t t) nil)
)

(qa
(with-temp-buffer
  (insert "ab") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer)) (reuse (match-data))
       (start (car reuse)) (end (cadr reuse)))
    (re-search-forward "b")
    (let ((out (match-data t reuse t)))
      (list (eq out reuse) (qa275-normalize out source)
	    (list (marker-position start) (marker-position end)
		  (eq (marker-buffer start) source)
		  (eq (marker-buffer end) source))
	    (eq (car out) start)))))
'(t (2 3 (buffer t)) (nil nil nil nil) nil)
)

(qa
(with-temp-buffer
  (insert "ab") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer))
       (reuse (append (match-data) (list (point-marker) 'old)))
       (start (car reuse)) (end (cadr reuse)) (extra (nth 2 reuse)))
    (string-match "b" "b")
    (let ((out (match-data t reuse nil)))
      (list (eq out reuse) out
	    (mapcar
	     (lambda (m)
	       (list (marker-position m) (eq (marker-buffer m) source)))
	     (list start end extra))))))
'(t (0 1 nil nil) ((1 t) (2 t) (2 t)))
)

(qa
(with-temp-buffer
  (insert "ab") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer))
       (reuse (append (match-data) (list (point-marker) 'old)))
       (start (car reuse)) (end (cadr reuse)) (extra (nth 2 reuse)))
    (string-match "b" "b")
    (let ((out (match-data t reuse t)))
      (list (eq out reuse) out
	    (mapcar
	     (lambda (m)
	       (list (marker-position m) (eq (marker-buffer m) source)))
	     (list start end extra))))))
'(t (0 1 nil nil) ((nil nil) (nil nil) (nil nil)))
)

(qa
(with-temp-buffer
  (insert "a") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer)) (reuse (match-data))
       (start (car reuse)))
    (set-match-data nil)
    (list (match-data t reuse nil) (qa275-normalize reuse source)
	  (marker-position start) (eq (marker-buffer start) source))))
'((nil nil) (nil nil) 1 t)
)

(qa
(with-temp-buffer
  (insert "a") (goto-char 1) (re-search-forward "a")
  (let*
      ((source (current-buffer)) (reuse (match-data))
       (start (car reuse)))
    (set-match-data nil)
    (list (match-data t reuse t) (qa275-normalize reuse source)
	  (marker-position start) (eq (marker-buffer start) source))))
'((nil nil) (nil nil) nil nil)
)

(qa
(let ((reuse (make-list 10 'old)))
  (string-match "\\(a\\)?\\(b\\)\\(c\\)?" "b")
  (let ((out (match-data t reuse))) (list (eq out reuse) out)))
'(t (0 1 nil nil 0 1 nil nil nil nil))
)


(princ (qa--done))
