;; case-table + downcase/upcase — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - case-table-p is a char-table subtype.
;;   - downcase/upcase/capitalize transform chars/strings.
;;   - *-region fns operate on the region.

;; == surface ==
(qa (fboundp 'case-table-p) t)
(qa (fboundp 'set-case-table) t)
(qa (fboundp 'standard-case-table) t)
(qa (fboundp 'current-case-table) t)
(qa (fboundp 'downcase) t)
(qa (fboundp 'upcase) t)
(qa (fboundp 'capitalize) t)
(qa (fboundp 'capitalize-word) t)
(qa (fboundp 'upcase-word) t)
(qa (fboundp 'downcase-word) t)
(qa (fboundp 'upcase-region) t)
(qa (fboundp 'downcase-region) t)
(qa (fboundp 'capitalize-region) t)
(qa (fboundp 'upcase-initials) t)
(qa (fboundp 'upcase-initials-region) t)

;; == case-table-p on case tables ==
(qa (case-table-p (current-case-table)) t)
(qa (case-table-p (standard-case-table)) t)
(qa (case-table-p nil) nil)
(qa (case-table-p "abc") nil)
(qa (case-table-p (make-hash-table)) nil)

;; == set-case-table returns the table ==
(let ((ct (set-case-table (current-case-table))))
  (qa (case-table-p ct) t))

;; == downcase / upcase on strings ==
(qa (downcase "HeLLo") "hello")
(qa (upcase "HeLLo") "HELLO")
(qa (downcase "") "")
(qa (upcase "") "")
(qa (downcase "123") "123")
(qa (upcase "123") "123")

;; == capitalize ==
(qa (capitalize "hello") "Hello")
(qa (capitalize "hello world") "Hello World")
(qa (capitalize "HELLO") "Hello")
(qa (capitalize "") "")

;; == upcase-initials — first letter of each word ==
(qa (upcase-initials "hello world") "Hello World")
(qa (upcase-initials "HELLO") "HELLO")
(qa (upcase-initials "") "")

;; == char-level upcase/downcase ==
(qa (upcase ?a) ?A)
(qa (downcase ?A) ?a)
(qa (upcase ?Z) ?Z)
(qa (downcase ?z) ?z)
(qa (upcase ?1) ?1)
(qa (downcase ?1) ?1)

;; == upcase-region on buffer ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "hello")
        (upcase-region (point-min) (point-max))
        (qa (buffer-string) "HELLO"))
    (kill-buffer buf)))

;; == downcase-region ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "HELLO")
        (downcase-region (point-min) (point-max))
        (qa (buffer-string) "hello"))
    (kill-buffer buf)))

;; == capitalize-region ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "hello world")
        (capitalize-region (point-min) (point-max))
        (qa (buffer-string) "Hello World"))
    (kill-buffer buf)))

;; == upcase-initials-region ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "hello world")
        (upcase-initials-region (point-min) (point-max))
        (qa (buffer-string) "Hello World"))
    (kill-buffer buf)))

;; == upcase-word in buffer (moves point) ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "hello world")
        (goto-char (point-min))
        (upcase-word 1)
        (qa (buffer-string) "HELLO world")
        (qa (point) 6))
    (kill-buffer buf)))

;; == downcase-word in buffer ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "HELLO WORLD")
        (goto-char (point-min))
        (downcase-word 1)
        (qa (buffer-string) "hello WORLD"))
    (kill-buffer buf)))

;; == capitalize-word in buffer ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "hello world")
        (goto-char (point-min))
        (capitalize-word 1)
        (qa (buffer-string) "Hello world"))
    (kill-buffer buf)))

;; == upcase-word with negative arg — backwards ==
(let ((buf (generate-new-buffer " *c*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "hello world")
        (goto-char (point-max))
        (upcase-word -1)
        (qa (buffer-string) "hello WORLD"))
    (kill-buffer buf)))

;; == non-ASCII case is sensible ==
(qa (upcase "héllo") "HÉLLO")
(qa (downcase "HÉLLO") "héllo")

(princ (qa--done))
