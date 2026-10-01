;; kbd.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - (kbd "C-a") returns a string containing the control char.
;;   - (kbd "M-x") returns a VECTOR of one event (meta).
;;   - (kbd "<f3>") returns a vector of one symbol.
;;   - (kbd "<RET>") returns a string with \r.
;;   - key-description returns "C-s" etc.

;; == surface ==
(qa (fboundp 'kbd) t)
(qa (fboundp 'key-parse) t)
(qa (fboundp 'key-valid-p) t)
(qa (fboundp 'key-description) t)
(qa (fboundp 'single-key-description) t)
(qa (fboundp 'read-kbd-macro) t)
(qa (fboundp 'format-kbd-macro) t)
(qa (fboundp 'edit-kbd-macro) t)
(qa (fboundp 'keymap--menu-item-binding) t)

;; == kbd produces strings for simple letters ==
(let ((k (kbd "a")))
  (qa (type-of k) 'string)
  (qa (length k) 1)
  (qa (aref k 0) ?a))

;; == kbd with control chars — STRING ==
(let ((k (kbd "C-a")))
  (qa (type-of k) 'string)
  (qa (length k) 1)
  (qa (aref k 0) 1))

(let ((k (kbd "C-x")))
  (qa (type-of k) 'string)
  (qa (length k) 1)
  (qa (aref k 0) 24))

;; == kbd multi-key string ==
(let ((k (kbd "C-x C-k")))
  (qa (type-of k) 'string)
  (qa (length k) 2)
  (qa (aref k 0) 24)
  (qa (aref k 1) 11))

;; == kbd with META — VECTOR ==
(let ((k (kbd "M-x")))
  (qa (type-of k) 'vector)
  (qa (length k) 1)
  ;; 27 | meta-bit
  (qa (aref k 0) 134217848))

;; == kbd with function keys — VECTOR of symbols ==
(let ((k (kbd "<f3>")))
  (qa (type-of k) 'vector)
  (qa (length k) 1)
  (qa (aref k 0) 'f3))

;; == kbd "<RET>" — GNU returns STRING with \r ==
(qa (let ((k (kbd "<RET>")))
      (list (type-of k) (aref k 0)))
    '(string 13))
;; == kbd "<tab>" ==
;; == kbd "<tab>" — VECTOR (not \t) ==
(let ((k (kbd "<tab>")))
  (qa (type-of k) 'vector)
  (qa (aref k 0) 'tab))
;; == kbd "C-M-s" — VECTOR ==
(let ((k (kbd "C-M-s")))
  (qa (type-of k) 'vector)
  (qa (length k) 1)
  ;; C|M|s — meta+ctrl on 's
  (qa (aref k 0) 134217747))

;; == key-description returns the string form ==
(qa (key-description [24]) "C-x")
(qa (key-description [?\C-s]) "C-s")
(qa (key-description [?\M-x]) "M-x")
(qa (key-description [?\r]) "RET")
(qa (key-description [?\t]) "TAB")
;; multi-char keys
(qa (key-description [24 11]) "C-x C-k")

;; == single-key-description ==
(qa (single-key-description ?a) "a")
(qa (single-key-description 24) "C-x")
(qa (single-key-description ?\C-s) "C-s")
(qa (single-key-description ?\M-x) "M-x")
(qa (single-key-description ?\r) "RET")
(qa (single-key-description ?\t) "TAB")
(qa (single-key-description ?\e) "ESC")
;; with NO-ANGLES — returns bare chars for function keys
(qa (single-key-description 'f3 t) "f3")

;; == key-valid-p on strings ==
(qa (key-valid-p "C-a") t)
(qa (key-valid-p "M-x") t)
(qa (key-valid-p "<f3>") t)
(qa (key-valid-p "") nil)
(qa (key-valid-p nil) nil)

;; == key-parse parses a key description into events ==
(let ((v (key-parse "C-x C-f")))
  (qa (type-of v) 'vector)
  (qa (length v) 2)
  (qa (aref v 0) 24)
  (qa (aref v 1) 6))

;; == format-kbd-macro ==
(let ((s (format-kbd-macro "abc")))
  (qa (stringp s) t))

;; == read-kbd-macro on simple strings ==
(let ((v (read-kbd-macro "abc")))
  (qa (sequencep v) t))

(princ (qa--done))
