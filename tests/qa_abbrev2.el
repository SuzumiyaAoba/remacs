;; Abbrev tables / obarray / plist — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - make-abbrev-table produces an obarrayp.
;;   - abbrev-symbol returns a fresh uninterned symbol — compare by
;;     symbol-name, not eq.
;;   - abbrev-expansion returns the expansion string.
;;   - abbrev-table-empty-p: t iff no abbrev defined.
;;   - copy-abbrev-table duplicates.
;;   - clear-abbrev-table empties.
;;   - obarray-size returns the BUCKET count (not symbol count).
;;   - plist-member returns the tail starting at the key.
;;   - lax-plist-get / lax-plist-put exist.
;;   - define-abbrev TABLE NAME EXPANSION &optional HOOK &rest PROPS;
;;     legacy 6-arg form is (… HOOK COUNT SYSTEM).

;; == abbrev fns ==
(qa (fboundp 'abbrev-mode) t)
(qa (fboundp 'abbrev-get) t)
(qa (fboundp 'abbrev-put) t)
(qa (fboundp 'expand-abbrev) t)
(qa (fboundp 'unexpand-abbrev) t)
(qa (fboundp 'define-abbrev) t)
(qa (fboundp 'define-abbrev-table) t)
(qa (fboundp 'make-abbrev-table) t)
(qa (fboundp 'abbrev-table-p) t)
(qa (fboundp 'abbrev-table-get) t)
(qa (fboundp 'abbrev-table-put) t)
(qa (fboundp 'abbrev-table-empty-p) t)
(qa (fboundp 'copy-abbrev-table) t)
(qa (fboundp 'clear-abbrev-table) t)
(qa (fboundp 'list-abbrevs) t)
(qa (fboundp 'edit-abbrevs) t)
(qa (fboundp 'edit-abbrevs-mode) t)
(qa (fboundp 'kill-all-abbrevs) t)
(qa (fboundp 'inverse-add-abbrev) t)
(qa (fboundp 'write-abbrev-file) t)
(qa (fboundp 'read-abbrev-file) t)
(qa (fboundp 'quietly-read-abbrev-file) t)

;; == abbrev vars ==
(qa (boundp 'abbrev-file-name) t)
(qa (boundp 'abbrev-all-caps) t)
(qa (boundp 'only-global-abbrevs) t)
(qa (boundp 'local-abbrev-table) t)
(qa (boundp 'global-abbrev-table) t)
(qa (boundp 'abbrev-minor-mode-table-alist) t)
(qa (boundp 'abbrev-table-name-list) t)

;; == make-abbrev-table → obarray ==
(qa (obarrayp (make-abbrev-table)) t)

;; == define-abbrev + abbrev-symbol / -expansion ==
(let ((tab (make-abbrev-table)))
  (define-abbrev tab "hw" "hello world")
  ;; abbrev-symbol returns an uninterned symbol — compare by name
  (qa (symbol-name (abbrev-symbol "hw" tab)) "hw")
  (qa (abbrev-expansion "hw" tab) "hello world"))
;; abbrev-table-empty-p
(qa (abbrev-table-empty-p (make-abbrev-table)) t)
(let ((t2 (make-abbrev-table)))
  (define-abbrev t2 "x" "y")
  (qa (abbrev-table-empty-p t2) nil))
;; abbrev-get: legacy SYSTEM arg at position 6, keyword :system
(let ((t3 (make-abbrev-table)))
  (define-abbrev t3 "hw" "hello world" nil nil t)
  (qa (abbrev-get (abbrev-symbol "hw" t3) :system) t))
;; define-abbrev-table: name + abbrev list
(let ((tab-name 'qa--tab))
  (eval `(define-abbrev-table ',tab-name '(("hw" "hello world"))))
  (qa (symbol-name (abbrev-symbol "hw" (symbol-value tab-name))) "hw"))
;; on global-abbrev-table
(define-abbrev global-abbrev-table "qw" "qwerty")
(qa (symbol-name (abbrev-symbol "qw" global-abbrev-table)) "qw")
(qa (abbrev-expansion "qw" global-abbrev-table) "qwerty")
;; copy-abbrev-table
(let* ((t4 (make-abbrev-table))
       (t5 (progn (define-abbrev t4 "x" "y") (copy-abbrev-table t4))))
  (qa (symbol-name (abbrev-symbol "x" t5)) "x"))
;; clear-abbrev-table empties
(let ((t6 (make-abbrev-table)))
  (define-abbrev t6 "x" "y")
  (clear-abbrev-table t6)
  (qa (abbrev-table-empty-p t6) t))

;; == obarray-* ==
(qa (fboundp 'obarray-make) t)
(qa (fboundp 'obarray-clear) t)
(qa (fboundp 'obarray-size) t)
(qa (fboundp 'obarrayp) t)
(let ((o (obarray-make)))
  ;; obarray-size returns the BUCKET count, not symbol count
  (qa (natnump (obarray-size o)) t)
  (intern "abc" o)
  ;; interned symbol in `o' is a distinct symbol object — compare by name
  (qa (symbol-name (intern-soft "abc" o)) "abc")
  (obarray-clear o)
  (qa (intern-soft "abc" o) nil))

;; == plist-*
(let ((pl '(:a 1 :b 2)))
  (qa (plist-get pl :a) 1)
  (qa (plist-get pl :b) 2)
  (qa (plist-get pl :c) nil)
  ;; plist-member returns the tail at the key
  (qa (plist-member pl :b) '(:b 2))
  (qa (plist-member pl :z) nil))
;; plist-put returns new list
(let ((pl (list :a 1)))
  (qa (plist-put pl :b 2) '(:a 1 :b 2)))
;; lax-plist-get / lax-plist-put
(qa (fboundp 'lax-plist-get) t)
(qa (fboundp 'lax-plist-put) t)
(let ((pl (list :a 1)))
  (qa (lax-plist-get pl :a) 1)
  (qa (lax-plist-get pl :z) nil))

(princ (qa--done))
