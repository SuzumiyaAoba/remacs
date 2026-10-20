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

;; Window parameter keys/order, minibuffer navigation and pixels (#152).
(defmacro qa152-window (&rest body)
  `(let ((qa152-w (split-window)))
     (unwind-protect (progn ,@body) (delete-window qa152-w))))
(qa (window-parameter nil 5) nil)
(qa (qa152-window
      (set-window-parameter qa152-w 'a 1)
      (set-window-parameter qa152-w 'b 2)
      (let ((before (window-parameters qa152-w)))
        (set-window-parameter qa152-w 'a 3)
        (set-window-parameter qa152-w 'b nil)
        (list before (window-parameters qa152-w))))
    '(((b . 2) (a . 1)) ((b) (a . 3))))
(qa (qa152-window
      (let ((key (copy-sequence "key")))
        (set-window-parameter qa152-w key 'string)
        (set-window-parameter qa152-w 5 'number)
        (set-window-parameter qa152-w nil 'nil-key)
        (list (window-parameter qa152-w key)
              (window-parameter qa152-w (copy-sequence key))
              (window-parameter qa152-w 5)
              (window-parameter qa152-w nil)))) '(string nil number nil-key))
(qa (qa152-window
      (set-window-parameter qa152-w 'a 1)
      (let ((copy (window-parameters qa152-w)))
        (setcdr (car copy) 'changed)
        (setcar (car copy) 'renamed)
        (window-parameter qa152-w 'a))) 1)
(qa (let ((w (split-window)))
      (delete-window w)
      (set-window-parameter w 5 'dead-value)
      (window-parameter w 5)) 'dead-value)
(qa-err-data (window-parameter 5 "key") 'wrong-type-argument '(windowp 5))
(qa-err-data (set-window-parameter 5 "key" 1) 'wrong-type-argument '(windowp 5))
(qa (list (window-left nil) (window-right nil)
          (window-left (minibuffer-window)) (window-right (minibuffer-window))) '(nil nil nil nil))
(qa (qa152-window
      (let ((first (selected-window)) (mini (minibuffer-window)))
        (list (eq (window-right first) qa152-w)
              (eq (window-left qa152-w) first)
              (eq (window-next-sibling (frame-root-window)) mini)
              (null (window-right (frame-root-window)))
              (null (window-left mini))))) '(t t t t t))
(qa (qa152-window
      (let ((first (selected-window)) (mini (minibuffer-window)))
        (list (eq (next-window first) qa152-w)
              (eq (next-window qa152-w) first)
              (eq (next-window qa152-w t) mini)
              (eq (next-window mini) first)
              (eq (previous-window mini) qa152-w)
              (eq (previous-window first t) mini)
              (eq (next-window mini t) first)
              (eq (next-window qa152-w 'never) first)
              (eq (previous-window first 'never) qa152-w)))) '(t t t t t t t t t))
(qa (let ((w (selected-window)) (mini (minibuffer-window)))
      (mapcar (lambda (frames)
                (and (eq (next-window w t frames) mini)
                     (eq (previous-window mini nil frames) w)))
              (list nil t 'visible 0 'bogus (selected-frame)))) '(t t t t t t))
(qa-err-data (next-window 5 t) 'wrong-type-argument '(window-live-p 5))
(qa-err-data (previous-window 5 t) 'wrong-type-argument '(window-live-p 5))
(qa (window-text-pixel-size nil nil nil nil nil t) '(0 . 1))
(qa (window-text-pixel-size nil nil nil nil nil 'mode-line) '(0 . 1))
(qa (window-text-pixel-size (minibuffer-window) nil nil nil nil t) '(0 . 0))
(qa (let ((mode-line-format nil) (header-line-format "H") (tab-line-format "T"))
      (mapcar (lambda (mode) (window-text-pixel-size nil nil nil nil nil mode))
              '(nil t mode-line header-line tab-line bogus)))
    '((0 . 0) (0 . 2) (0 . 0) (0 . 1) (0 . 1) (0 . 0)))
(qa (window-text-pixel-size nil '(1 . 0)) '(0 . 0))
(qa (window-text-pixel-size nil '(1 . 2)) '(0 0 1))
(qa (window-text-pixel-size nil '(1 . -2)) '(0 0 1))
(qa-err-data (window-text-pixel-size nil "x") 'wrong-type-argument '(integer-or-marker-p "x"))
(qa-err-data (window-text-pixel-size nil 1.5) 'wrong-type-argument '(integer-or-marker-p 1.5))
(qa-err-data (window-text-pixel-size nil '("x" . 0)) 'wrong-type-argument '(integer-or-marker-p "x"))
(qa-err-data (window-text-pixel-size nil '(1 . bad)) 'wrong-type-argument '(fixnump bad))
(qa-err-data (window-text-pixel-size nil nil "x") 'wrong-type-argument '(integer-or-marker-p "x"))
(qa-err-data (window-text-pixel-size nil nil '(1 . 0)) 'wrong-type-argument '(integer-or-marker-p (1 . 0)))
(qa-err-data (window-text-pixel-size nil (make-marker)) 'error '("Marker does not point anywhere"))
(qa-err-data (window-pixel-edges 5) 'error '("5 is not a valid window"))
(qa (qa152-window (listp (window-pixel-edges (window-parent qa152-w)))) t)
(qa (qa152-window
      (set-window-parameter qa152-w 'mode-line-format 'none)
      (window-text-pixel-size qa152-w nil nil nil nil t)) '(0 . 0))
(qa (qa152-window
      (let ((mode-line-format nil))
        (set-window-parameter qa152-w 'mode-line-format "M")
        (window-text-pixel-size qa152-w nil nil nil nil t))) '(0 . 1))
(qa (with-temp-buffer
      (setq-local mode-line-format nil)
      (let ((current (current-buffer)))
        (list (window-text-pixel-size nil nil nil nil nil t)
              (eq current (current-buffer))))) '((0 . 1) t))
(qa (window-text-pixel-size nil '(1 . 4294967296)) '(0 . 0))
(princ (qa--done))
