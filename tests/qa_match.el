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

(princ (qa--done))
