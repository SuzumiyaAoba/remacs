;; Characters — GNU 31.1 -Q --batch ground truth.
;;
;; GNU truths pinned:
;;   - chars are just integers in [0, #x3FFFFF]; max-char = 4194303.
;;   - char< does NOT exist in GNU 31.1 (use <); char-equal is 2-ary
;;     and case-insensitive under the default case-fold-search=t.
;;   - char-to-string returns a MULTIBYTE string for non-ASCII.
;;   - string-to-char "" => 0.
;;   - char-width: ASCII 1, TAB obeys tab-width (8), CJK 2, NUL 2.
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
(qa (char-width ?α) 2)
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
(qa-err-data (put-char-code-property 4194304 'name "invalid")
             'wrong-type-argument '(characterp 4194304))
(qa-err-data (put-char-code-property -1 'name "invalid")
             'wrong-type-argument '(characterp -1))
(qa-err-data (put-char-code-property 18446744073709551616 'name "invalid")
             'wrong-type-argument '(fixnump 18446744073709551616))

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
