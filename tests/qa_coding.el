;; Coding systems — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - utf-8 / utf-8-unix / utf-16 / iso-8859-1 / cp1251/1252 are valid.
;;   - coding-system-eol-type: unix=0, dos=1, mac=2.
;;   - coding-system-base of an eol variant → the base system.
;;   - aliases of utf-8: (utf-8 mule-utf-8 cp65001).
;;   - detect-coding-string returns a list of detected systems.
;;   - check-coding-system returns the coding system symbol.
;;   - coding-system-eol-type-mnemonic 'utf-8 → ":".
;;   - default-buffer-file-coding-system is not bound in batch (-Q)
;;     when locale is POSIX.

;; == existence ==
(qa (listp (coding-system-list)) t)
(qa (listp (coding-system-list t)) t)
(qa (coding-system-p 'utf-8) t)
(qa (coding-system-p 'utf-8-unix) t)
(qa (coding-system-p 'iso-8859-1) t)
(qa (coding-system-p 'utf-16) t)
(qa (coding-system-p 'no-such-coding) nil)

;; == plist queries ==
(qa (coding-system-get 'utf-8 'name) 'utf-8)
(qa (coding-system-get 'utf-8 'mnemonic) 85)        ; ?U
(qa (coding-system-get 'utf-8 'coding-type) 'utf-8)

;; == eol-type ==
(qa (coding-system-eol-type 'utf-8-unix) 0)
(qa (coding-system-eol-type 'utf-8-dos) 1)
(qa (coding-system-eol-type 'utf-8-mac) 2)
(qa (coding-system-eol-type-mnemonic 'utf-8) ":")

;; == detect-coding-string ==
(qa (equal (detect-coding-string "hello") '(undecided)) t)

;; == check-coding-system ==
(qa (check-coding-system 'utf-8) 'utf-8)

;; == aliases ==
(qa (coding-system-aliases 'utf-8) '(utf-8 mule-utf-8 cp65001))

;; == other coding systems exist ==
(qa (coding-system-p 'cp1251) t)
(qa (coding-system-p 'cp1252) t)
(qa (coding-system-p 'iso-8859-2) t)
(qa (coding-system-p 'mac-roman) t)

;; == coding-system-base ==
(qa (coding-system-base 'utf-8-unix) 'utf-8)

;; == vars ==
(qa (boundp 'buffer-file-coding-system) t)
(qa (boundp 'default-buffer-file-coding-system) nil)

;; == coding-system-p on non-symbol → nil ==
(qa (coding-system-p 42) nil)

;; == decode / encode roundtrip ==
(let ((s (decode-coding-string
          (encode-coding-string "abc 日本語" 'utf-8)
          'utf-8)))
  (qa s "abc 日本語"))

;; cp1251 encodes Cyrillic; decode roundtrip
(let ((enc (encode-coding-string "привет" 'cp1251)))
  (qa (decode-coding-string enc 'cp1251) "привет"))

;; == find-operation-coding-system ==
(qa (fboundp 'find-operation-coding-system) t)

(princ (qa--done))
