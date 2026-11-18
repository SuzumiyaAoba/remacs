;; Characters — GNU 31.1 -Q --batch ground truth.
;;
;; GNU truths pinned:
;;   - chars are just integers in [0, #x3FFFFF]; max-char = 4194303.
;;   - char< does NOT exist in GNU 31.1 (use <); char-equal is 2-ary
;;     and case-insensitive under the default case-fold-search=t.
;;   - char-to-string returns a MULTIBYTE string for non-ASCII.
;;   - string-to-char "" => 0.
;;   - char-width: ASCII 1, Greek alpha 1, TAB obeys tab-width (8), CJK 2, NUL 2.
;;   - char-syntax returns the syntax code integer.
;;   - ucs-normalize-* are autoloads (fboundp t before require).

;; == surface ==
(qa (fboundp 'characterp) t)
(qa (fboundp 'char-equal) t)
(qa (fboundp 'char<) nil)
(qa (fboundp 'char-to-string) t)
(qa (fboundp 'string-to-char) t)
(qa (fboundp 'char-width) t)
(qa (fboundp 'max-char) t)
(qa (fboundp 'char-syntax) t)
(qa (fboundp 'get-char-code-property) t)
(qa (fboundp 'multibyte-char-to-unibyte) t)
(qa (fboundp 'unibyte-char-to-multibyte) t)
(qa (fboundp 'char-or-string-p) t)
(qa (fboundp 'ucs-normalize-NFC-string) t)

;; == characterp: chars are fixnums in range ==
(qa (characterp ?a) t)
(qa (characterp 97) t)
(qa (characterp "a") nil)
(qa (characterp (max-char)) t)
(qa (characterp (1+ (max-char))) nil)
(qa (characterp -1) nil)
(qa (characterp 5.0) nil)
(qa (max-char) 4194303)

;; == char-equal: case-insensitive under default case-fold-search=t ==
(qa (char-equal ?a ?a) t)
(qa (char-equal ?a ?b) nil)
(qa (char-equal ?a ?A) t)          ; case-fold-search is t in batch
(qa (char-equal ?a 97) t)
(qa-err (char-equal ?a ?a ?a) 'wrong-number-of-arguments)
(qa (let ((case-fold-search t)) (char-equal ?a ?A)) t)
;; bound to nil it becomes case-sensitive
(qa (let ((case-fold-search nil)) (char-equal ?a ?A)) nil)
;; compare-strings with IGNORE-CASE=t returns t for equal
(qa (compare-strings "a" nil nil "A" nil nil t) t)

;; == char comparison via < ==
(qa (< ?a ?b) t)
(qa (< ?b ?a) nil)
(qa (< ?a ?a) nil)
(qa (< ?a ?b ?c) t)
(qa (< ?a ?c ?b) nil)

;; == char-to-string ==
(qa (char-to-string ?a) "a")
(qa (char-to-string 65) "A")
(qa (char-to-string 945) "α")
(qa (multibyte-string-p (char-to-string 945)) t)
(qa-err (char-to-string -1) 'wrong-type-argument)
(qa-err (char-to-string #x400000) 'wrong-type-argument)
(qa-err (char-to-string 1.5) 'wrong-type-argument)
(qa-err (char-to-string "a") 'wrong-type-argument)

;; == string-to-char ==
(qa (string-to-char "abc") 97)
(qa (string-to-char "") 0)
(qa (string-to-char "αβ") 945)
(qa-err (string-to-char 5) 'wrong-type-argument)

;; == aref on strings returns char codes ==
(qa (aref "abc" 0) 97)
(qa (aref "abc" 2) 99)
(qa (aref "αβ" 0) 945)
(qa (characterp (aref "abc" 1)) t)
(qa-err (aref "abc" 5) 'args-out-of-range)
(qa-err (aref "abc" -1) 'args-out-of-range)

;; == char-width ==
(qa (char-width ?a) 1)
(qa (char-width ?\t) 8)
(qa (char-width ?α) 1)
(qa (char-width ?中) 2)
(qa (char-width 0) 2)
;; tab width follows tab-width var
(qa (let ((tab-width 4)) (char-width ?\t)) 4)

;; == char-syntax (raw code integers) ==
(qa (char-syntax ?a) 119)     ; w word
(qa (char-syntax ?\s) 32)     ; space
(qa (char-syntax ?\() 40)     ; open paren
(qa (char-syntax ?\") 34)     ; string quote
(qa (char-syntax ?') 39)      ; expression prefix

;; == string constructor ==
(qa (string ?a ?b ?c) "abc")
(qa (string) "")
(qa (string 945) "α")

;; == char-or-string-p ==
(qa (char-or-string-p ?a) t)
(qa (char-or-string-p "x") t)
(qa (char-or-string-p 5.0) nil)
(qa (char-or-string-p 'sym) nil)

;; == case conversion on chars ==
(qa (downcase ?A) 97)
(qa (upcase ?a) 65)
(qa (downcase ?1) 49)
(qa (upcase ?ß) 7838)         ; ẞ
(qa (downcase ?Α) 945)

;; == multibyte/unibyte roundtrip ==
(qa (multibyte-char-to-unibyte ?a) 97)
(qa (unibyte-char-to-multibyte 97) 97)

;; == get-char-code-property ==
(qa (get-char-code-property ?a 'name) "LATIN SMALL LETTER A")
(qa (get-char-code-property ?A 'general-category) 'Lu)
(qa (get-char-code-property ?a 'general-category) 'Ll)
;; Built-in descriptions also require the canonical interned property and
;; value symbols; same-named uninterned symbols are distinct names.
(qa (char-code-property-description (make-symbol "general-category") 'Lu) nil)
(qa (char-code-property-description 'general-category (make-symbol "Lu")) nil)
(qa (char-code-property-description (make-symbol "bidi-class") 'L) nil)
;; Built-in defaults belong to the registered property symbol, not to any
;; uninterned symbol that happens to have the same printed name.
(qa (get-char-code-property ?A (make-symbol "name")) nil)
(qa (get-char-code-property ?A 42) nil)
(qa
 (let ((char-code-property-table (make-char-table 'char-code-property-table)))
   (put-char-code-property ?A 42 'non-symbol-property)
   (get-char-code-property ?A 42))
 'non-symbol-property)
(qa-err-data (get-char-code-property -1 'name)
             'wrong-type-argument '(characterp -1))
(qa-err-data (get-char-code-property -1 42)
             'wrong-type-argument '(characterp -1))
(qa-err-data (get-char-code-property 4194304 'name)
             'wrong-type-argument '(characterp 4194304))

;; A registered property must use a char-code-property table whose name
;; slot agrees with the property.  Other char-table subtypes are rejected.
(qa-err
 (define-char-code-property (make-symbol "qa58-wrong-subtype")
                            (make-char-table 'syntax-table))
 'error)
(qa-err
 (let ((table (make-char-table 'char-code-property-table)))
   (set-char-table-extra-slot table 0 'qa58-different-property)
   (define-char-code-property 'qa58-expected-property table))
 'error)

;; When an unregistered property later gains a table, the table replaces
;; its old fallback values for lookups and writes.
(qa
 (let* ((char-code-property-alist char-code-property-alist)
        (property (make-symbol "qa58-late-table"))
        (table (make-char-table 'char-code-property-table)))
   (put-char-code-property 65 property 'fallback-value)
   (set-char-table-extra-slot table 0 property)
   (define-char-code-property property table)
   (put-char-code-property 65 property 'table-value)
   (get-char-code-property 65 property))
 'table-value)

;; Property writes accept all internal characters, but never wrap or
;; clamp invalid integer codes into another character's table entry.
(let* ((plain (make-symbol "qa58-plain"))
       (registered (make-symbol "qa58-table"))
       (table (make-char-table 'char-code-property-table))
       (char-code-property-alist char-code-property-alist))
  (set-char-table-extra-slot table 0 registered)
  (define-char-code-property registered table)
  (dolist (prop (list plain registered))
    (dolist (ch '(0 1114111 1114112 4194303))
      (let ((value (list 'qa58 ch)))
        (qa (eq (put-char-code-property ch prop value) value) t)
        (qa (eq (get-char-code-property ch prop) value) t)))
    (let ((sentinel (list 'qa58-preserved)))
      (put-char-code-property 0 prop sentinel)
      (dolist (ch (list -1 4194304 4294967296 most-positive-fixnum))
        (qa-err-data (put-char-code-property ch prop 'invalid)
                     'wrong-type-argument (list 'characterp ch))
        (qa (eq (get-char-code-property 0 prop) sentinel) t)))
    (qa (put-char-code-property 4194303 prop nil) nil)
    (qa (get-char-code-property 4194303 prop) nil))
  (put-char-code-property 0 registered 'preserved)
  (qa-err-data (put-char-code-property 18446744073709551616 registered 'invalid)
               'wrong-type-argument '(characterp 18446744073709551616))
  (qa (get-char-code-property 0 registered) 'preserved)
  ;; User-defined setters decide their own valid domain; the native
  ;; guards must not intercept arbitrary callbacks.
  (let ((observed nil))
    (set-char-table-extra-slot table 2
                              (lambda (ch value table) (setq observed ch)))
    (qa (put-char-code-property 4194304 registered 'callback-value) 'callback-value)
    (qa observed 4194304)))

;; Adding a new char-code property assigns char-code-property-alist through
;; the normal Lisp setter; a watcher error aborts the registration.
(defun qa58-char-code-alist-watcher (_symbol _value operation _where)
  (when (eq operation 'set)
    (signal 'arith-error '("char-code property watcher"))))
(qa (let* ((before char-code-property-alist)
           (char-code-property-alist (copy-sequence before))
           (name (make-symbol "qa58-watched"))
           (table (make-char-table 'char-code-property-table)))
      (set-char-table-extra-slot table 0 name)
      (add-variable-watcher 'char-code-property-alist
                            #'qa58-char-code-alist-watcher)
      (unwind-protect
          (list (condition-case err
                    (progn (define-char-code-property name table) 'ok)
                  (error (car err)))
                (equal char-code-property-alist before))
        (remove-variable-watcher 'char-code-property-alist
                                 #'qa58-char-code-alist-watcher)))
    '(arith-error t))

;; Charset aliases update the C-backed charset list without invoking its
;; Lisp variable watchers.
(defvar qa58-charset-list-events nil)
(defun qa58-charset-list-watcher (_symbol _value operation _where)
  (push operation qa58-charset-list-events))
(qa (let ((qa58-charset-list-events nil))
      (add-variable-watcher 'charset-list #'qa58-charset-list-watcher)
      (unwind-protect
          (progn
            (define-charset-alias 'qa58-audit-charset-alias 'ascii)
            (list (reverse qa58-charset-list-events)
                  (eq (car charset-list) 'qa58-audit-charset-alias)))
        (remove-variable-watcher 'charset-list #'qa58-charset-list-watcher)))
    '(nil t))

;; Defining a table mutates an available vector slot with `aset'; it does
;; not rebind translation-table-vector or notify its variable watchers.
(defun qa58-translation-table-vector-watcher (_symbol _value operation _where)
  (push operation qa58-translation-table-vector-events))
(qa (let ((translation-table-vector (make-vector 16 nil))
          (qa58-translation-table-vector-events nil)
          (symbol (make-symbol "qa58-translation-table")))
      (add-variable-watcher 'translation-table-vector
                            #'qa58-translation-table-vector-watcher)
      (unwind-protect
          (let ((original translation-table-vector))
            (define-translation-table symbol '((?a . ?b)))
            (list (eq original translation-table-vector)
                  (reverse qa58-translation-table-vector-events)
                  (= (get symbol 'translation-table-id) 0)
                  (eq (cdr (aref translation-table-vector 0))
                      (get symbol 'translation-table))))
        (remove-variable-watcher 'translation-table-vector
                                 #'qa58-translation-table-vector-watcher)))
    '(t nil t t))
(qa (let ((translation-table-vector (vector (cons 'qa58-occupied nil)))
          (qa58-translation-table-vector-events nil)
          (symbol (make-symbol "qa58-grown-translation-table")))
      (add-variable-watcher 'translation-table-vector
                            #'qa58-translation-table-vector-watcher)
      (unwind-protect
          (let ((original translation-table-vector))
            (define-translation-table symbol '((?a . ?b)))
            (list (eq original translation-table-vector)
                  (reverse qa58-translation-table-vector-events)
                  (length translation-table-vector)
                  (get symbol 'translation-table-id)
                  (eq (cdr (aref translation-table-vector 1))
                      (get symbol 'translation-table))))
        (remove-variable-watcher 'translation-table-vector
                                 #'qa58-translation-table-vector-watcher)))
    '(nil (set) 2 1 t))
(qa-err (let ((translation-table-vector "x"))
          (define-translation-table (make-symbol "qa58-invalid-vector")
                                    '((?a . ?b))))
        'wrong-type-argument)
(qa-err (let ((translation-table-vector nil))
          (define-translation-table (make-symbol "qa58-empty-vector")
                                    '((?a . ?b))))
        'args-out-of-range)

;; `make-translation-table' processes each mapping before it reaches a dotted
;; tail, so an earlier AREF error takes precedence over the later listp error.
(qa-err-data
 (make-translation-table (cons (cons 65 66) 'qa58-dotted-tail))
 'wrong-type-argument '(listp qa58-dotted-tail))
(qa-err-data
 (make-translation-table (cons (cons 65 "bad") 'qa58-dotted-tail))
 'wrong-type-argument '(fixnump "bad"))
(qa-err-data (make-translation-table 5)
             'wrong-type-argument '(listp 5))

;; The Lisp accessor returns the registered cdr directly; C's uniprop_table
;; performs lazy loading and validates Unicode table subtypes separately.
(qa (let* ((table (make-char-table 'qa58-generic-purpose))
           (char-code-property-alist
            (cons (cons 'qa58-generic-prop table) char-code-property-alist)))
      (eq (unicode-property-table-internal 'qa58-generic-prop)
          (cdr (assq 'qa58-generic-prop char-code-property-alist))))
    t)
(qa-err
 (let* ((property (make-symbol "qa58-generic-get-prop"))
        (table (make-char-table 'qa58-generic-purpose))
        (char-code-property-alist
         (cons (cons property table) char-code-property-alist)))
   (get-char-code-property 65 property))
 'args-out-of-range)
(qa (let ((char-code-property-alist
           (cons (cons 'qa58-missing-prop "qa58-no-such-unicode-property")
                 char-code-property-alist)))
      (unicode-property-table-internal 'qa58-missing-prop))
    "qa58-no-such-unicode-property")

;; `char-code-property-alist' is authoritative over the Rust lookup cache;
;; removing a registration makes the internal lookup return nil immediately.
(qa
 (let* ((property (make-symbol "qa58-removed-property"))
        (table (make-char-table 'char-code-property-table))
        (char-code-property-alist char-code-property-alist))
   (set-char-table-extra-slot table 0 property)
   (define-char-code-property property table)
   (let ((registered (unicode-property-table-internal property)))
     (setf (alist-get property char-code-property-alist) nil)
     (list (eq registered table)
           (unicode-property-table-internal property))))
 '(t nil))

(defvar qa58-stale-property-file-loaded nil)
(qa
 (let* ((property 'qa58-interned-stale-backing)
        (file (make-temp-file "qa58-stale-property-" nil ".el"))
        (table (make-char-table 'char-code-property-table))
        (char-code-property-alist char-code-property-alist)
        (qa58-stale-property-file-loaded nil))
   (unwind-protect
       (progn
         (with-temp-file file
           (insert "(setq qa58-stale-property-file-loaded t)"))
         (set-char-table-extra-slot table 0 property)
         (define-char-code-property property file)
         (setcdr (assq property char-code-property-alist) table)
         (aset table 65 'table-value)
         (list (get-char-code-property 65 property)
               qa58-stale-property-file-loaded))
     (define-char-code-property property table)
     (delete-file file)))
 '(table-value nil))

;; Unregistered properties use the Lisp char-code-property-table itself;
;; direct table edits and `put-char-code-property' must share that plist.
(qa
 (let* ((property (make-symbol "qa58-direct-fallback"))
        (char-code-property-table (make-char-table 'char-code-property-table)))
   (aset char-code-property-table 65 (list property 'direct-value))
   (let ((read (get-char-code-property 65 property))
         (written (put-char-code-property 66 property 'written-value)))
     (list (eq read 'direct-value)
           (eq written 'written-value)
           (eq (car (aref char-code-property-table 66)) property)
           (eq (cadr (aref char-code-property-table 66)) 'written-value)
           (eq (get-char-code-property 66 property) 'written-value))))
 '(t t t t t))

;; Reloading a file-backed property after its table has registered must not
;; replace the materialized table with the filename again.
(qa
 (let* ((property (make-symbol "qa58-reloaded-table"))
        (table (make-char-table 'char-code-property-table))
        (char-code-property-alist char-code-property-alist))
   (set-char-table-extra-slot table 0 property)
   (define-char-code-property property table)
   (define-char-code-property property "qa58-missing-reload-file")
   (eq (unicode-property-table-internal property) table))
 t)

;; Property keys are symbols compared with `eq'; distinct uninterned symbols
;; that share a printed name do not share a registration.
(qa
 (let* ((first (make-symbol "qa58-same-name"))
        (second (make-symbol "qa58-same-name"))
        (interned 'qa58-same-name)
        (table (make-char-table 'char-code-property-table))
        (char-code-property-alist char-code-property-alist))
   (set-char-table-extra-slot table 0 first)
   (aset table 65 'first-property-value)
   (define-char-code-property first table)
   (list (eq (caar char-code-property-alist) first)
         (eq (unicode-property-table-internal first) table)
         (unicode-property-table-internal second)
         (unicode-property-table-internal interned)
         (get-char-code-property 65 first)
         (get-char-code-property 65 second)
         (get-char-code-property 65 interned)))
 '(t t nil nil first-property-value nil nil))

(qa-err-data (put-char-code-property 4194304 'name "invalid")
             'wrong-type-argument '(characterp 4194304))
(qa-err-data (put-char-code-property -1 'name "invalid")
             'wrong-type-argument '(characterp -1))
(qa-err-data (put-char-code-property 18446744073709551616 'name "invalid")
             'wrong-type-argument '(fixnump 18446744073709551616))

;; `dolist' in `make-translation-table' visits an improper list's tail as
;; nil once, which then fails the mapping's `aref' index check.
(qa-err-data
 (make-translation-table (cons (list (cons ?a ?b)) 'bad-tail))
 'wrong-type-argument '(fixnump nil))
(qa-err-data (make-translation-table 'bad-tail)
             'wrong-type-argument '(listp bad-tail))

;; `define-translation-table' evaluates the old vector before `make-vector'
;; runs; an advised constructor cannot replace that captured first argument.
(defun qa58-translation-vector-side-effect (original length initial)
  (setq translation-table-vector (vector (cons 'qa58-replaced nil)))
  (funcall original length initial))
(qa
 (let ((translation-table-vector (vector (cons 'qa58-occupied nil)))
       (symbol (make-symbol "qa58-vector-evaluation-order")))
   (advice-add 'make-vector :around #'qa58-translation-vector-side-effect)
   (unwind-protect
       (progn
         (define-translation-table symbol '((?a . ?b)))
         (list (eq (car (aref translation-table-vector 0)) 'qa58-occupied)
               (eq (car (aref translation-table-vector 1)) symbol)))
     (advice-remove 'make-vector #'qa58-translation-vector-side-effect)))
 '(t t))

;; == ucs-normalize (autoloaded in 31.1) ==
(qa (ucs-normalize-NFC-string "é") "é")           ; already composed
(qa (ucs-normalize-NFC-string "é") "é")          ; e + combining acute → é
(qa (ucs-normalize-NFD-string "é") "é")          ; é → e + combining acute
(qa (ucs-normalize-NFC-string "abc") "abc")
(qa (ucs-normalize-NFKC-string "①") "1")          ; compat: circled 1 → 1
(qa (ucs-normalize-NFKD-string "①") "1")
;; Darwin HFS variants
(qa (ucs-normalize-HFS-NFC-string "é") "é")
(qa (ucs-normalize-HFS-NFD-string "é") "é")

;; == equality/identity ==
(qa (eq ?a 97) t)
(qa (eql ?a 97) t)
(qa (equal ?a 97) t)
(qa (= ?a 97) t)

(princ (qa--done))
