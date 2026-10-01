;; Abbrev tables & expansion — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - global-abbrev-table/local-abbrev-table are abbrev-table-p → t.
;;   - define-abbrev-table NAME ABBREVS &optional DOC creates a new table
;;     stored on NAME's value cell; abbrev-table-name-list grows.
;;   - define-abbrev TABLE ABBREV EXPANSION defines ABBREV inside TABLE.
;;   - abbrev-expansion returns the expansion string; abbrev-symbol the
;;     abbrev's symbol.  Unknown abbrev → nil.
;;   - expand-abbrev expands the word before point; returns the abbrev
;;     symbol on success, nil when nothing to expand.
;;   - unexpand-abbrev undoes the last expansion, restoring the abbrev.
;;   - clear-abbrev-table empties a table (expansions become nil).
;;   - abbrev-put/abbrev-get take the abbrev SYMBOL (not a string).
;;   - write-abbrev-file + quietly-read-abbrev-file roundtrip the
;;     global table.

;; == table predicates ==
(qa (abbrev-table-p global-abbrev-table) t)
(qa (abbrev-table-p local-abbrev-table) t)
(qa (abbrev-table-p nil) nil)
(qa (abbrev-table-p 'sym) nil)
;; name list contains global-abbrev-table
(qa (if (memq 'global-abbrev-table abbrev-table-name-list) t nil) t)

;; == define-abbrev-table ==
(define-abbrev-table 'qa-atab nil "qa table")
(qa (abbrev-table-p (symbol-value 'qa-atab)) t)
;; the new table is registered in the name list
(qa (if (memq 'qa-atab abbrev-table-name-list) t nil) t)
;; a second define-abbrev-table replaces content
(define-abbrev-table 'qa-atab nil "qa table 2")
(qa (abbrev-table-p (symbol-value 'qa-atab)) t)

;; == define-abbrev / abbrev-expansion / abbrev-symbol ==
(define-abbrev qa-atab "qq" "expanded")
(qa (abbrev-expansion "qq" qa-atab) "expanded")
;; abbrev-symbol interns in the TABLE's own obarray → compare names
(qa (symbol-name (abbrev-symbol "qq" qa-atab)) "qq")
;; unknown abbrev → nil
(qa (abbrev-expansion "nope" qa-atab) nil)
(qa (abbrev-symbol "nope" qa-atab) nil)
;; expansion in a different table is unaffected
(define-abbrev global-abbrev-table "ga" "global-abbrev")
(qa (abbrev-expansion "ga" global-abbrev-table) "global-abbrev")
;; redefine changes expansion
(define-abbrev qa-atab "qq" "expanded-2")
(qa (abbrev-expansion "qq" qa-atab) "expanded-2")

;; == abbrev-put / abbrev-get properties (3-arg, abbrev SYMBOL) ==
(define-abbrev qa-atab "rp" "rep")
(abbrev-put 'rp :count 5)
(qa (abbrev-get 'rp :count) 5)
(abbrev-put 'rp :custom-prop "v")
(qa (abbrev-get 'rp :custom-prop) "v")
;; missing prop → nil
(qa (abbrev-get 'rp :missing) nil)

;; == expand-abbrev: word before point ==
(with-temp-buffer
  (setq local-abbrev-table qa-atab)
  (insert "qq")
  ;; returns the abbrev symbol (table's obarray) → compare by name
  (qa (symbol-name (expand-abbrev)) "qq")
  (qa (buffer-string) "expanded-2"))
;; no abbrev before point → nil, buffer unchanged
(with-temp-buffer
  (setq local-abbrev-table qa-atab)
  (insert "zz")
  (qa (expand-abbrev) nil)
  (qa (buffer-string) "zz"))
;; expansion after whitespace/word boundary
(with-temp-buffer
  (setq local-abbrev-table qa-atab)
  (insert "start qq")
  ;; returns the abbrev symbol (table's obarray) → compare by name
  (qa (symbol-name (expand-abbrev)) "qq")
  (qa (buffer-string) "start expanded-2"))
(with-temp-buffer
  (setq local-abbrev-table qa-atab)
  (insert "xqqx")
  (qa (expand-abbrev) nil)
  (qa (buffer-string) "xqqx"))

;; == unexpand-abbrev restores the abbrev text ==
(with-temp-buffer
  (setq local-abbrev-table qa-atab)
  (insert "qq")
  (expand-abbrev)
  (unexpand-abbrev)
  (qa (buffer-string) "qq"))

;; == clear-abbrev-table ==
(let ((t2 (copy-abbrev-table qa-atab)))
  (qa (abbrev-expansion "qq" t2) "expanded-2")
  (clear-abbrev-table t2)
  (qa (abbrev-expansion "qq" t2) nil))
;; copy-abbrev-table preserves entries
(let ((t3 (copy-abbrev-table qa-atab)))
  (qa (abbrev-expansion "rp" t3) "rep"))

;; == write-abbrev-file + read roundtrip ==
(let ((f "/tmp/qa_abbrevs_test.el"))
  (write-abbrev-file f)
  (qa (file-exists-p f) t)
  ;; wipe the global table, reload
  (let ((saved (copy-abbrev-table global-abbrev-table)))
    (clear-abbrev-table global-abbrev-table)
    (qa (abbrev-expansion "ga" global-abbrev-table) nil)
    (quietly-read-abbrev-file f)
    (qa (abbrev-expansion "ga" global-abbrev-table) "global-abbrev"))
  (delete-file f))

;; == save-abbrevs / abbrev-file-name defaults ==
(qa save-abbrevs t)
(qa abbrev-file-name "~/.emacs.d/abbrev_defs")
(qa (boundp 'abbrevs-changed) t)

;; == only-global-abbrevs ==
(qa only-global-abbrevs nil)

;; == abbrev-prefix-mark / insert-abbrev-table-description exist ==
(qa (fboundp 'abbrev-prefix-mark) t)
(qa (fboundp 'insert-abbrev-table-description) t)
(qa (fboundp 'prepare-abbrev-list-buffer) t)
(qa (fboundp 'edit-abbrevs-mode) t)

;; == inverse-add-abbrev reads interactively — untestable; fboundp only ==
(qa (fboundp 'inverse-add-abbrev) t)
(qa (fboundp 'add-abbrev) t)

(princ (qa--done))
