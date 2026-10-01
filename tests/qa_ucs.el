;; UCS names / normalization — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - ucs-names returns a hash-table of name → codepoint.
;;   - char-from-name "LATIN SMALL LETTER A" → 97.
;;   - char-from-name "GRINNING FACE" → 128512.
;;   - ucs-normalize-NFD/NFC strings split/compose accents.
;;   - char-code-property is NOT a fn in 31.1 batch.
;;   - ucs-normalize-*-string are in ucs-normalize (require 'ucs-normalize).

;; == surface ==
(qa (fboundp 'ucs-names) t)
(qa (fboundp 'char-from-name) t)
(qa (fboundp 'ucs-normalize-string) t)
(qa (fboundp 'ucs-normalize-NFD-string) t)
(qa (fboundp 'ucs-normalize-NFC-string) t)
(qa (fboundp 'ucs-normalize-NFKD-string) t)
(qa (fboundp 'ucs-normalize-NFKC-string) t)
(qa (fboundp 'char-code-property) nil)

;; == char-from-name ==
(qa (char-from-name "LATIN SMALL LETTER A") 97)
(qa (char-from-name "LATIN CAPITAL LETTER A") 65)
(qa (char-from-name "DIGIT ZERO") 48)
(qa (char-from-name "GRINNING FACE") 128512)
;; unknown name → nil
(qa (char-from-name "NOT A REAL CHAR NAME") nil)

;; == ucs-names ==
(let ((tbl (ucs-names)))
  (qa (hash-table-p tbl) t)
  ;; LATIN SMALL LETTER A → 97
  (qa (gethash "LATIN SMALL LETTER A" tbl) 97)
  (qa (gethash "LATIN CAPITAL LETTER A" tbl) 65))

;; == normalize round-trips ==
(require 'ucs-normalize)
;; é NFC → single codepoint 233
(qa (ucs-normalize-NFC-string "e\u0301") "é")
;; é NFD → decomposed e + combining accent
(let ((d (ucs-normalize-NFD-string "é")))
  (qa (length d) 2)
  (qa (string-to-list d) '(101 769)))
;; roundtrip
(qa (ucs-normalize-NFC-string (ucs-normalize-NFD-string "é")) "é")

;; ucs-normalize-NFKC folds compat chars
;; ﬁ (U+FB01 LATIN SMALL LIGATURE FI) → "fi" under NFKC
(let ((s (ucs-normalize-NFKC-string "\uFB01")))
  (qa s "fi"))

;; ucs-normalize-NFKD keeps decomposed + compat
(let ((s (ucs-normalize-NFKD-string "e\u0301")))
  ;; stays decomposed (already NFD)
  (qa (length s) 2))

;; ucs-names is a hash-table (lazy init)
(qa (hash-table-p (ucs-names)) t)

;; char-from-name uses ucs-names internally
;; Roundtrip: name → char
(qa (char-from-name "HYPHEN-MINUS") 45)
(qa (char-from-name "SPACE") 32)
;; ucs-names contains several ASCII names
(let ((tbl (ucs-names)))
  ;; At minimum a few names
  (qa (> (hash-table-count tbl) 1000) t))

(princ (qa--done))
