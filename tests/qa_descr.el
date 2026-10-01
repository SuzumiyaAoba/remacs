;; descr-text / character description — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - char-displayable-p returns t / 'unicode.
;;   - unicode-property-table-internal returns a char-table (raw access
;;     requires the accessor; aref/char-table-range can return garbage).
;;   - get-char-code-property is the safe accessor.
;;   - what-cursor-position returns a string.

;; == surface ==
(qa (fboundp 'describe-char) t)
(qa (fboundp 'describe-char-elisp) nil)
(qa (fboundp 'describe-char-map) nil)
(qa (fboundp 'describe-char-categories) nil)
(qa (fboundp 'describe-char-display) nil)
(qa (fboundp 'describe-char-prefer-string) nil)
(qa (fboundp 'char-displayable-p) t)
(qa (fboundp 'describe-char-custom-data) nil)
(qa (fboundp 'describe-char-fold-equivalences) t)
(qa (fboundp 'describe-char-eldoc) t)
(qa (fboundp 'what-cursor-position) t)
(qa (fboundp 'describe-text-properties) t)
(qa (fboundp 'describe-text-sexp) nil)
(qa (fboundp 'describe-text-done) nil)
(qa (fboundp 'unicode-property-table-internal) t)
(qa (fboundp 'get-char-code-property) t)

;; == char-displayable-p — ASCII always displayable ==
(qa (char-displayable-p ?a) t)
(qa (char-displayable-p ?A) t)
(qa (char-displayable-p ?0) t)
(qa (char-displayable-p ?\s) t)
(qa (char-displayable-p ?\n) t)
(qa (char-displayable-p 0) t)
;; on batch, emoji → 'unicode (has a name but can't render)
(qa (char-displayable-p #x1F600) 'unicode)
(qa (char-displayable-p ?中) 'unicode)

;; == unicode-property-table-internal returns a char-table ==
(let ((tbl (unicode-property-table-internal 'name)))
  (qa (char-table-p tbl) t)
  (qa (char-table-subtype tbl) 'char-code-property-table))

;; == get-char-code-property is the safe accessor ==
(qa (get-char-code-property ?a 'name) "LATIN SMALL LETTER A")
(qa (get-char-code-property ?A 'name) "LATIN CAPITAL LETTER A")
(qa (get-char-code-property ?0 'name) "DIGIT ZERO")
;; 'bidi-class returns a symbol
(qa (symbolp (get-char-code-property ?a 'bidi-class)) t)
(qa (get-char-code-property ?a 'bidi-class) 'L)
(qa (get-char-code-property ?\u05D0 'bidi-class) 'R)  ; Hebrew letter

;; == what-cursor-position — batch-safe, returns a string ==
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  ;; GNU returns the "Char: ..." info string even in batch.
  (let ((s (what-cursor-position)))
    (qa (stringp s) t)))

;; == describe-text-properties is a fn ==
(qa (fboundp 'describe-text-properties) t)

;; == unicode-property-table-internal caches — same table ==
(let ((t1 (unicode-property-table-internal 'name))
      (t2 (unicode-property-table-internal 'name)))
  (qa (char-table-p t1) t)
  (qa (char-table-p t2) t))

;; == ucs-names is a hash-table of name → codepoint ==
(let ((tbl (ucs-names)))
  (qa (hash-table-p tbl) t)
  (qa (gethash "LATIN SMALL LETTER A" tbl) 97))

(princ (qa--done))
