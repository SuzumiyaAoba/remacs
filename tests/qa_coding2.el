;; Coding-system domain — GNU 31.1 pins (verified).

;; == predicates ==
(qa (coding-system-p 'utf-8) t)
(qa (coding-system-p 'nope) nil)
(qa (coding-system-p 'raw-text) t)
(qa (coding-system-p 'emacs-internal) t)
(qa (coding-system-p 'no-conversion) t)
(qa (coding-system-p 'binary) t)
;; unix-eol/dos-eol/mac-eol are NOT coding systems
(qa (coding-system-p 'unix-eol) nil)
(qa (coding-system-p 'dos-eol) nil)
(qa (coding-system-p 'mac-eol) nil)

;; == list / aliases ==
(qa (listp (coding-system-list)) t)
(qa (if (memq 'utf-8 (coding-system-list)) t nil) t)
;; utf-8 aliases: exactly the 3 known aliases on this build
(qa (coding-system-aliases 'utf-8) '(utf-8 mule-utf-8 cp65001))

;; == base / eol-type ==
(qa (coding-system-base 'utf-8-unix) 'utf-8)
(qa (coding-system-base 'utf-8-dos) 'utf-8)
(qa (coding-system-base 'utf-8-mac) 'utf-8)
(qa (coding-system-base 'utf-8) 'utf-8)
(qa (coding-system-eol-type 'utf-8) '[utf-8-unix utf-8-dos utf-8-mac])
(qa (coding-system-eol-type 'utf-8-unix) 0)
(qa (coding-system-eol-type 'utf-8-dos) 1)
(qa (coding-system-eol-type 'utf-8-mac) 2)

;; == plist / get ==
(qa (plist-get (coding-system-plist 'utf-8) :mnemonic) 85)
(qa (plist-get (coding-system-plist 'utf-8) :coding-type) 'utf-8)
(qa (plist-get (coding-system-plist 'utf-8) :category)
    'coding-category-utf-8)
(qa (coding-system-get 'utf-8 :mnemonic) 85)
(qa (coding-system-get 'utf-8 :coding-type) 'utf-8)
(qa (if (memq 'unicode (coding-system-get 'utf-8 :charset-list)) t nil) t)

;; == check-coding-system ==
(qa (check-coding-system 'utf-8) 'utf-8)
(qa-err (check-coding-system 'nope-xyz) 'coding-system-error)

;; == check-coding-systems-region on pure-ASCII → nil ==
(with-temp-buffer
  (insert "abc")
  (qa (check-coding-systems-region 1 4 '(utf-8)) nil))

;; == find-coding-systems-region/string: all-ascii → (undecided) ==
(with-temp-buffer
  (insert "abc")
  (qa (find-coding-systems-region 1 4) '(undecided)))
(qa (find-coding-systems-string "abc") '(undecided))

;; == detect-coding-region: with non-nil HIGHEST GNU returns the bare ==
;;    symbol 'undecided. ==
(with-temp-buffer
  (insert "abc")
  (qa (detect-coding-region 1 4 t) 'undecided)
)
(qa (fboundp 'detect-coding-system) nil)
(qa (fboundp 'detect-coding-region) t)
(qa (fboundp 'detect-coding-string) t)

;; == encode/decode roundtrip ==
(qa (decode-coding-string (encode-coding-string "abc" 'utf-8) 'utf-8)
    "abc")
;; GNU: multibyte char encodes to 3 bytes.
(qa (string-bytes (encode-coding-string "あ" 'utf-8)) 3)
(qa (decode-coding-string (string-as-unibyte "\xE3\x81\x82") 'utf-8)
    "あ")
(with-temp-buffer
  (insert "abc")
  (encode-coding-region 1 4 'utf-8)
  (qa (buffer-string) "abc"))

;; == eol conversion ==
(qa (coding-system-change-eol-conversion 'utf-8 1) 'utf-8-dos)
(qa (coding-system-change-eol-conversion 'utf-8 0) 'utf-8-unix)

;; == auto-coding alists ==
(qa (listp auto-coding-alist) t)
(qa (listp auto-coding-regexp-alist) t)
(qa (listp auto-coding-functions) t)
(qa (fboundp 'find-coding-systems-from-filename) nil)
(qa (listp (find-coding-systems-for-charsets '(unicode))) t)

;; == conversion hooks ==
(qa (fboundp 'coding-system-pre-write-conversion) t)
(qa (fboundp 'coding-system-post-read-conversion) t)
(qa (fboundp 'coding-system-change-eol-conversion) t)

;; == terminal/keyboard CS ==
(qa (coding-system-p (terminal-coding-system)) t)
(qa (coding-system-p (keyboard-coding-system)) t)
(qa (fboundp 'selection-coding-system) nil)

;; == coding-system alists / defaults ==
(qa (listp file-coding-system-alist) t)
(qa (listp process-coding-system-alist) t)
(qa (listp network-coding-system-alist) t)
(qa default-process-coding-system '(utf-8-unix . utf-8-unix))

;; == misc ==
(qa (fboundp 'eol-mnemonic-unix) nil)
(qa (coding-system-p (intern "utf-8-unix")) t)

;; Issue #81: omitted COUNT defaults to nil, including when a hit exists.
(with-temp-buffer
  (insert "abcé文é")
  (qa (unencodable-char-position 1 7 'us-ascii) 4)
  (qa (unencodable-char-position 1 7 'us-ascii nil) 4)
  (qa (unencodable-char-position 5 7 'us-ascii) 5)
  (qa (unencodable-char-position 1 4 'us-ascii) nil)
  (qa (unencodable-char-position 4 4 'us-ascii) nil)
  (qa (unencodable-char-position 1 7 'utf-8) nil)
  (qa (unencodable-char-position (copy-marker 1) (copy-marker 7) 'us-ascii) 4)
  (qa (unencodable-char-position 1 7 'us-ascii 0) '(4 5 6))
  (qa (unencodable-char-position 1 7 'us-ascii 1) '(4))
  (qa (unencodable-char-position 1 7 'us-ascii 2) '(4 5))
  (qa (unencodable-char-position 1 7 'us-ascii 10) '(4 5 6))
  ;; Validation remains lazy: a bad COUNT matters only after finding a hit.
  (qa (unencodable-char-position 1 4 'us-ascii t) nil)
  (qa-err-data (unencodable-char-position 1 7 'us-ascii t)
               'wrong-type-argument '(wholenump t))
  (qa (funcall (byte-compile (lambda ()
                  (unencodable-char-position 1 7 'us-ascii)))) 4))
(qa (with-temp-buffer (unencodable-char-position 1 1 'us-ascii)) nil)
(qa (unencodable-char-position 0 3 'us-ascii nil "aéb") 1)

(princ (qa--done))
