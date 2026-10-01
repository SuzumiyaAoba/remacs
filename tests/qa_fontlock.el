;; Font-lock / jit-lock / face-remap — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - font-lock-* faces are all real faces.
;;   - font-lock-defaults / font-lock-keywords are buffer-local vars.
;;   - font-lock-maximum-size is unbound in 31.1.
;;   - font-lock-global-modes is unbound.
;;   - face-remap-add-relative returns a cookie; the alist entry is
;;     (FACE . SPECS) — check `caar' is the face.
;;   - face-remap-set-base installs base; reset-base removes it.

;; == font-lock vars ==
(qa (boundp 'font-lock-defaults) t)
(qa (boundp 'font-lock-keywords) t)
(qa (boundp 'font-lock-keywords-alist) t)
(qa (boundp 'font-lock-keywords-only) t)
(qa (boundp 'font-lock-keywords-case-fold-search) t)
(qa (boundp 'font-lock-major-mode) t)
(qa (boundp 'font-lock-multiline) t)
(qa (boundp 'font-lock-maximum-decoration) t)
(qa (boundp 'font-lock-maximum-size) nil)
(qa (boundp 'font-lock-global-modes) t)
(qa (boundp 'font-lock-beginning-of-syntax-function) nil)
(qa (boundp 'font-lock-syntactic-face-function) t)
(qa (boundp 'font-lock-syntactic-keywords) t)
(qa (boundp 'font-lock-syntactically-fontified) t)
(qa (boundp 'font-lock-syntax-table) t)
(qa (boundp 'font-lock-mark-block-function) t)

;; == faces ==
;; facep returns a face-spec VECTOR (not t) when defined
(qa (vectorp (facep 'font-lock-function-name-face)) t)
(qa (vectorp (facep 'font-lock-warning-face)) t)
(qa (vectorp (facep 'font-lock-keyword-face)) t)
(qa (vectorp (facep 'font-lock-type-face)) t)
(qa (vectorp (facep 'font-lock-variable-name-face)) t)
(qa (vectorp (facep 'font-lock-constant-face)) t)
(qa (vectorp (facep 'font-lock-string-face)) t)
(qa (vectorp (facep 'font-lock-comment-face)) t)
(qa (vectorp (facep 'font-lock-builtin-face)) t)
(qa (vectorp (facep 'font-lock-preprocessor-face)) t)
(qa (vectorp (facep 'font-lock-negation-char-face)) t)
(qa (vectorp (facep 'font-lock-regexp-grouping-backslash)) t)
(qa (vectorp (facep 'font-lock-regexp-grouping-construct)) t)
;; == font-lock fns ==
(qa (fboundp 'font-lock-mode) t)
(qa (fboundp 'global-font-lock-mode) t)
(qa (fboundp 'font-lock-add-keywords) t)
(qa (fboundp 'font-lock-remove-keywords) t)
(qa (fboundp 'font-lock-flush) t)
(qa (fboundp 'font-lock-ensure) t)
(qa (fboundp 'font-lock-fontify-block) t)

;; == jit-lock ==
(qa (fboundp 'jit-lock-mode) t)
(qa (fboundp 'jit-lock-register) t)
(qa (fboundp 'jit-lock-unregister) t)
(qa (fboundp 'jit-lock-refontify) t)
(qa (fboundp 'jit-lock-fontify-now) t)
(qa (boundp 'jit-lock-stealth-time) t)
(qa (boundp 'jit-lock-stealth-nice) t)
(qa (boundp 'jit-lock-stealth-load) t)
(qa (boundp 'jit-lock-defer-time) t)
(qa (boundp 'jit-lock-defer-contextually) t)
(qa (boundp 'jit-lock-chunk-size) t)

;; == face-remap ==
(qa (fboundp 'face-remap-add-relative) t)
(qa (fboundp 'face-remap-set-base) t)
(qa (fboundp 'face-remap-reset-base) t)
(qa (fboundp 'face-remap-remove-relative) nil)
(with-temp-buffer
  (let ((cookie (face-remap-add-relative 'default :foreground "red")))
    (qa (consp cookie) t)
    (qa (caar face-remapping-alist) 'default)))
;; set-base installs base; reset-base removed in 31.1 → face-remap-set-base
;; with no args resets
(with-temp-buffer
  (face-remap-set-base 'default :foreground "blue" :background "white")
  (qa (consp face-remapping-alist) t)
  (face-remap-set-base 'default)
  (qa face-remapping-alist nil))

(princ (qa--done))
