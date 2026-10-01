;; rect.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - kill-rectangle removes the columns and returns them.
;;   - yank-rectangle inserts them at point.
;;   - extract-rectangle returns the lines as a list.
;;   - clear-rectangle replaces the rect with spaces.
;;   - open-rectangle inserts whitespace (shifts right).
;;   - string-rectangle replaces the rect with a string.
;;   - string-insert-rectangle inserts a string without replacing.

;; == surface ==
(qa (fboundp 'rectangle-mark-mode) t)
(qa (fboundp 'copy-rectangle-to-register) t)
(qa (fboundp 'yank-rectangle) t)
(qa (fboundp 'open-rectangle) t)
(qa (fboundp 'close-rectangle) t)
(qa (fboundp 'clear-rectangle) t)
(qa (fboundp 'string-rectangle) t)
(qa (fboundp 'string-insert-rectangle) t)
(qa (fboundp 'delete-rectangle) t)
(qa (fboundp 'kill-rectangle) t)
(qa (fboundp 'delete-whitespace-rectangle) t)
(qa (fboundp 'extract-rectangle) t)
(qa (fboundp 'rectangle-number-lines) t)
;; NOT fns in 31.1 — most rectangle-* internals aren't fns
(qa (fboundp 'rectangle-mark-mode-maybe) nil)
(qa (fboundp 'rectangle-exchange-point-and-mark) nil)
(qa (fboundp 'rectangle-right-char) nil)
(qa (fboundp 'rectangle-left-char) nil)
(qa (fboundp 'rectangle-bounds) nil)
(qa (fboundp 'rectangle-dimensions) nil)
(qa (fboundp 'rectangle-position-as-coordinates) nil)
(qa (fboundp 'rectangle-coordinates-as-position) nil)
(qa (fboundp 'rectangle-region-as-string) nil)
(qa (fboundp 'copy-region-as-rectangle) nil)
(qa (fboundp 'rectangle-separate-window) nil)
(qa (fboundp 'rectangle-move-point) nil)

;; == kill-rectangle / yank-rectangle round-trip ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abcd\nefgh")
        ;; kill cols 0-3 (start=1, end=5 covers first 4 chars of first line)
        ;; but the rect is the intersection across all lines
        (let ((k (kill-rectangle 1 5)))
          ;; the killed rect = ["abcd"] only covers line 1's cols 0-3
          ;; because line 2 doesn't reach col 5 with content
          (qa k '("abcd"))
          ;; buffer has the remaining content
          (qa (buffer-string) "\nefgh")))
    (kill-buffer buf)))

;; == extract-rectangle returns the line slices ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abcd\nefgh\nijkl")
        ;; cols 1-2 of lines 1-2
        (qa (extract-rectangle 2 9) '("bc" "fg"))
        ;; extract doesn't modify the buffer
        (qa (buffer-string) "abcd\nefgh\nijkl"))
    (kill-buffer buf)))

;; == string-rectangle — replaces the rect ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abcd\nefgh")
        (string-rectangle 1 5 "XX")
        (qa (buffer-string) "XX\nefgh"))
    (kill-buffer buf)))

;; == open-rectangle — inserts whitespace ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abcd\nefgh")
        (open-rectangle 2 5)
        ;; cols 1-3 opened up: 'a' + '   ' + 'bcd'; line 2 unchanged
        (qa (buffer-string) "a   bcd\nefgh"))
    (kill-buffer buf)))

;; == clear-rectangle — replaces rect with spaces ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abcd\nefgh")
        (clear-rectangle 2 5)
        ;; 'a' + spaces + removed 'bcd'; line 2 unchanged (cols beyond)
        (qa (buffer-string) "a\nefgh"))
    (kill-buffer buf)))

;; == rectangle-number-lines — (BEG END &optional START-AT) ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc\ndef\nghi")
        (rectangle-number-lines 1 (point-max) 1)
        ;; lines numbered "N " prefix
        (qa (buffer-string) "1 abc\n2 def\n3 ghi"))
    (kill-buffer buf)))

;; == delete-whitespace-rectangle — deletes only trailing whitespace ==
;; Within the rect, only whitespace is removed; non-ws is preserved.
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "a   b\nc   d")
        (delete-whitespace-rectangle 1 5)
        ;; GNU: only whitespace removed; 'b'/'d' still there
        (qa (buffer-string) "a   b\nc   d"))
    (kill-buffer buf)))

;; == kill-rectangle followed by yank-rectangle restores it ==
(let ((buf (generate-new-buffer " *r*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abcd\nefgh")
        (let ((k (kill-rectangle 1 5)))
          (goto-char (point-min))
          (yank-rectangle)
          ;; same rect re-inserted — line 1 'abcd' returns
          (qa (string-prefix-p "abcd" (buffer-string)) t)))
    (kill-buffer buf)))

(princ (qa--done))
