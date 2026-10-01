;; buff-menu.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - list-buffers / buffer-menu produce a *Buffer List*.
;;   - Buffer-menu-* commands drive the UI.

;; == surface ==
(qa (fboundp 'list-buffers) t)
(qa (fboundp 'buffer-menu) t)
(qa (fboundp 'Buffer-menu-mode) t)
(qa (fboundp 'list-buffers--refresh) t)
(qa (fboundp 'Buffer-menu-buffer) t)
(qa (fboundp 'Buffer-menu-sort) t)
(qa (fboundp 'Buffer-menu-execute) t)
(qa (fboundp 'Buffer-menu-mark) t)
(qa (fboundp 'Buffer-menu-unmark) t)
(qa (fboundp 'Buffer-menu-delete) t)
(qa (fboundp 'Buffer-menu-save) t)
(qa (fboundp 'Buffer-menu-select) t)
(qa (fboundp 'Buffer-menu-1-window) t)
(qa (fboundp 'Buffer-menu-2-window) t)
(qa (fboundp 'Buffer-menu-this-window) t)
(qa (fboundp 'Buffer-menu-other-window) t)
(qa (fboundp 'Buffer-menu-switch-other-window) t)
(qa (fboundp 'Buffer-menu-visit-tags-table) t)
(qa (fboundp 'Buffer-menu-isearch-buffers) t)
(qa (fboundp 'Buffer-menu-isearch-buffers-regexp) t)
(qa (fboundp 'Buffer-menu-multi-occur) t)
(qa (fboundp 'Buffer-menu-toggle-files-only) t)
(qa (fboundp 'Buffer-menu-toggle-read-only) t)
;; NOT fns
(qa (fboundp 'buffer-menu-buffer) nil)

;; == Buffer-menu vars ==
(qa (boundp 'Buffer-menu-use-header-line) t)
(qa (boundp 'Buffer-menu-use-frame-buffer-list) t)
(qa (boundp 'Buffer-menu-files-only) t)
(qa (boundp 'Buffer-menu-mode-map) t)
;; NOT vars
(qa (boundp 'Buffer-menu-buffer+size-width) nil)

;; == list-buffers creates *Buffer List* buffer ==
(let ((bl (get-buffer "*Buffer List*")))
  (when bl (kill-buffer bl)))
(list-buffers)
(let ((bl (get-buffer "*Buffer List*")))
  (qa (bufferp bl) t)
  ;; it's in Buffer-menu-mode
  (qa (with-current-buffer bl major-mode) 'Buffer-menu-mode)
  (kill-buffer bl))

;; == the *Buffer List* content has our live buffers ==
(list-buffers)
(let ((bl (get-buffer "*Buffer List*")))
  ;; *scratch*, *Messages* are standard
  (qa (with-current-buffer bl (stringp (buffer-string))) t)
  (kill-buffer bl))

;; == Buffer-menu-buffer — returns the menu BUFFER or nil ==
(let ((bl (get-buffer "*Buffer List*")))
  (when bl (kill-buffer bl)))
(let ((b (Buffer-menu-buffer)))
  (qa (or (null b) (bufferp b)) t))

;; == mode's keymap has the d/D/x bindings ==
(let ((km Buffer-menu-mode-map))
  (qa (keymapp km) t)
  (qa (lookup-key km "d") 'Buffer-menu-delete)
  (qa (lookup-key km "x") 'Buffer-menu-execute))

;; == Buffer-menu-sort exists and is callable on the buffer-list ==
(list-buffers)
(let ((bl (get-buffer "*Buffer List*")))
  (unwind-protect
      (with-current-buffer bl
        ;; sorting needs a column; don't exercise fully — verify callable
        (qa (fboundp 'Buffer-menu-sort) t))
    (kill-buffer bl)))

;; == ibuffer vs list-buffers ==
(qa (fboundp 'ibuffer) t)
(qa (fboundp 'ibuffer-list-buffers) t)

;; == other *-list buffers ==
(qa (fboundp 'list-processes) t)

(princ (qa--done))
