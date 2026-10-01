;; window.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - selected-window is always live.
;;   - window-buffer returns a buffer.
;;   - window-minibuffer-p nil for normal windows.
;;   - window-parameter defaults nil.

;; == surface — predicates ==
(qa (fboundp 'windowp) t)
(qa (fboundp 'window-live-p) t)
(qa (fboundp 'window-minibuffer-p) t)
(qa (fboundp 'window-buffer) t)
(qa (fboundp 'window-frame) t)
(qa (fboundp 'window-height) t)
(qa (fboundp 'window-width) t)
(qa (fboundp 'window-total-height) t)
(qa (fboundp 'window-total-width) t)
(qa (fboundp 'window-body-height) t)
(qa (fboundp 'window-body-width) t)
(qa (fboundp 'window-pixel-height) t)
(qa (fboundp 'window-pixel-width) t)
(qa (fboundp 'window-left) t)
(qa (fboundp 'window-right) t)
(qa (fboundp 'window-start) t)
(qa (fboundp 'window-end) t)
(qa (fboundp 'window-point) t)
(qa (fboundp 'window-hscroll) t)
(qa (fboundp 'window-vscroll) t)
(qa (fboundp 'window-scroll-bars) t)
(qa (fboundp 'window-fringes) t)
(qa (fboundp 'window-margins) t)
(qa (fboundp 'window-parameter) t)
(qa (fboundp 'window-parameters) t)
(qa (fboundp 'window-use-time) t)
(qa (fboundp 'window-old-point) t)
(qa (fboundp 'window-min-size) t)
(qa (fboundp 'window-next-sibling) t)
(qa (fboundp 'window-prev-sibling) t)
;; window-minibuffer isn't a fn — the real name is minibuffer-window
(qa (fboundp 'window-minibuffer) nil)
(qa (fboundp 'minibuffer-window) t)
(qa (fboundp 'window-child) t)
(qa (fboundp 'window-left-child) t)
(qa (fboundp 'window-top-child) t)
(qa (fboundp 'window-min-size) t)
(qa (fboundp 'window-list) t)
;; NOT fns in 31.1
(qa (fboundp 'window-selected-p) nil)
(qa (fboundp 'window-top) nil)
(qa (fboundp 'window-bottom) nil)
(qa (fboundp 'window-divider-widths) nil)
(qa (fboundp 'window-set-parameter) nil)
(qa (fboundp 'window-delete) nil)
(qa (fboundp 'window-redisplay-end-trigger) nil)

;; == selected-window basics ==
(let ((w (selected-window)))
  (qa (windowp w) t)
  (qa (window-live-p w) t)
  (qa (window-minibuffer-p w) nil)
  (qa (bufferp (window-buffer w)) t)
  ;; frame
  (qa (framep (window-frame w)) t)
  ;; parameters
  (qa (window-parameters w) nil)
  ;; parameter lookup
  (qa (window-parameter w 'nonexistent) nil))

;; == window-list — in batch there's 1 window ==
(let ((wl (window-list)))
  (qa (listp wl) t)
  (qa (>= (length wl) 1) t)
  ;; every entry is a window
  (qa (seq-every-p #'windowp wl) t))

;; == window-minibuffer returns the minibuffer window ==
(let ((mw (minibuffer-window)))
  (qa (windowp mw) t)
  (qa (window-minibuffer-p mw) t))

;; == window-next-sibling — batch: selected-window's sibling is the minibuffer ==
(let ((w (selected-window))
      (mw (minibuffer-window)))
  (qa (window-next-sibling w) mw))

;; == window-prev-sibling — nil for the left-most sibling ==
(let ((w (selected-window)))
  (qa (window-prev-sibling w) nil))

;; == window-buffer returns the right buffer ==
(let ((buf (generate-new-buffer " *t*"))
      (w (selected-window)))
  (unwind-protect
      (progn
        (set-window-buffer w buf)
        (qa (eq (window-buffer w) buf) t))
    (kill-buffer buf)))

;; == window-start / window-end / window-point ==
(let ((w (selected-window)))
  ;; point-min ≤ start ≤ end ≤ point-max
  (qa (<= (window-start w) (window-end w)) t)
  (qa (<= (window-start w) (window-point w)) t)
  (qa (<= (window-point w) (window-end w)) t))

;; == window-use-time is a number ==
(let ((w (selected-window)))
  (qa (integerp (window-use-time w)) t))

;; == window-old-point — nil before first display ==
(let ((w (selected-window)))
  ;; a number (or nil if never displayed)
  (qa (or (null (window-old-point w))
          (integerp (window-old-point w)))
      t))

;; == frame vars ==
(qa (fboundp 'framep) t)
(qa (fboundp 'selected-frame) t)
(qa (fboundp 'frame-parameter) t)
(qa (fboundp 'frame-parameters) t)


;; == window-height / width are ints ==
(let ((w (selected-window)))
  (qa (integerp (window-height w)) t)
  (qa (integerp (window-width w)) t)
  (qa (integerp (window-total-height w)) t)
  (qa (integerp (window-total-width w)) t)
  (qa (integerp (window-body-height w)) t)
  (qa (integerp (window-body-width w)) t))

;; window-safe-p isn't a fn in 31.1
(qa (fboundp 'window-safe-p) nil)
(let ((sw (selected-window))
      (mw (minibuffer-window)))
  ;; they're different objects
  (qa (eq sw mw) nil))

(princ (qa--done))
