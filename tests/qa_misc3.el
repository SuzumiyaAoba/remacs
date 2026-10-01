;; Misc buffer/var/string utilities — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - buffer-local-boundp takes (SYMBOL BUFFER) — 2 args.
;;   - variable-at-point returns the symbol near point, 0 when none.
;;   - props-at-point is NOT a fn — use text-properties-at.
;;   - string-bytes on a multibyte char returns its UTF-8 length.
;;   - string-as-unibyte reinterprets the UTF-8 bytes (len stays 3).
;;   - buffer-multibyte-p is NOT a fn; enable-multibyte-characters is
;;     the variable to read.
;;   - last-buffer returns the most-recently-buried buffer.
;;   - posn-at-x-y returns a posn event list.
;;   - line-pixel-height in batch → 1.

;; == buffer-local-boundp: is the var BOUND in BUFFER (not local) ==
(defvar qa-lv 'global)
(qa (buffer-local-boundp 'qa-lv (current-buffer)) t)
;; unbound var → nil
(qa (buffer-local-boundp 'qa-never-bound-xyz (current-buffer)) nil)
;; local-variable-p is the real "buffer-local binding" check
(qa (local-variable-p 'qa-lv) nil)
(with-temp-buffer
  (setq-local qa-lv 'local)
  (qa (local-variable-p 'qa-lv) t))
;; local-variable-p + BUFFER arg
(qa (local-variable-p 'qa-lv (get-buffer "*scratch*")) nil)

;; == variable-at-point ==
(with-temp-buffer
  (insert "fill-column")
  (goto-char 5)
  (qa (variable-at-point) 'fill-column))
;; a function symbol is NOT a variable → 0
(with-temp-buffer
  (insert "car")
  (goto-char 2)
  (qa (variable-at-point) 0))
;; nothing before/after → 0
(with-temp-buffer
  (insert " ")
  (goto-char 1)
  (qa (variable-at-point) 0))

;; == props-at-point is NOT a fn; text-properties-at is the API ==
(qa (fboundp 'props-at-point) nil)
(with-temp-buffer
  (insert (propertize "abc" 'face 'bold))
  (qa (text-properties-at 1) '(face bold))
  (qa (get-text-property 1 'face (current-buffer)) 'bold))
;; no props → nil
(with-temp-buffer
  (insert "abc")
  (qa (text-properties-at 1) nil))

;; == string-bytes (UTF-8 byte length) ==
(qa (string-bytes "あ") 3)           ; multibyte → 3 bytes
(qa (string-bytes "ab") 2)
(qa (string-bytes "é") 2)
(qa (string-bytes "") 0)
;; multibyte list arg? string-bytes requires a string
(qa-err (string-bytes 'sym) 'wrong-type-argument)

;; == string-as-unibyte / string-as-multibyte ==
;; string-as-unibyte reinterprets UTF-8 bytes → length unchanged
(qa (string-bytes (string-as-unibyte "あ")) 3)
(qa (multibyte-string-p (string-as-unibyte "x")) nil)
(qa (multibyte-string-p (string-as-multibyte "x")) t)
;; unibyte→multibyte roundtrip: a raw byte string becomes multibyte
(qa (multibyte-string-p (string-as-multibyte (string-as-unibyte "x"))) t)

;; == enable-multibyte-characters / set-buffer-multibyte ==
(with-temp-buffer
  (qa enable-multibyte-characters t)
  (set-buffer-multibyte nil)
  (qa enable-multibyte-characters nil)
  (set-buffer-multibyte t)
  (qa enable-multibyte-characters t))
;; buffer-multibyte-p is NOT a fn
(qa (fboundp 'buffer-multibyte-p) nil)

;; == last-buffer ==
;; in batch the previous buffer is the temp buffer just left
(with-temp-buffer
  (rename-buffer "qa-tmp-b1" t)
  (qa (buffer-name (last-buffer)) "qa-tmp-b1"))

;; == posn-at-x-y batch shape ==
;; returns a posn list: (window pos-or-area (x . y) …)
(let ((p (posn-at-x-y 0 0)))
  (qa (listp p) t)
  (qa (windowp (car p)) t))
;; outside the frame → nil? batch frame is 80x24; coords beyond → probe:
;; keep the type check only (reliable across frames)
(qa (listp (posn-at-x-y 1 0)) t)

;; == line-pixel-height ==
;; batch → 1 (single-char line height)
(qa (line-pixel-height) 1)

;; == text-properties-at with a string arg ==
(qa (text-properties-at 0 (propertize "xy" 'k 'v)) '(k v))
(qa (text-properties-at 0 "xy") nil)

(princ (qa--done))
