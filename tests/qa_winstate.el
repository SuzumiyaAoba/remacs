;; window-state / frameset — window tree serialization & related geometry.
;; GNU -Q --batch: one live window (rows 0-23) plus minibuffer (row 24),
;; 1x1 char pixels.  `window-state-get' returns (HEADER TAG . SLOTS)
;; where HEADER is a min-size alist and TAG is leaf|vc|hc — NOT a frame
;; tag.  Many side-window helpers are not even defined in 31.1.

;; == functions that do not exist in GNU 31.1 (pin void-function) ==
(qa-err (window-side) 'void-function)
(qa-err (window-direction) 'void-function)
(qa-err (window-used-as-side-window) 'void-function)
(qa-err (window-display-buffer) 'void-function)
(qa-err (window-resize-pixelwise) 'void-function)
(qa-err (window-fit-window-to-buffer) 'void-function)
(qa-err (window-splits-invariant) 'void-function)
(qa-err (frame-last-window) 'void-function)
(qa-err (window-right-child) 'void-function)
(qa-err (window-iso-combined-p) 'void-function)
(qa-err (window-make-container) 'void-function)
(qa-err (delete-side-window) 'void-function)
(qa-err (window-inside-pixel-width) 'void-function)
(qa-err (window-inside-pixel-height) 'void-function)
(qa-err (window-chars-pixel-width) 'void-function)
(qa-err (window-scroll-bar-columns) 'void-function)
(qa-err (window-state-put-safe) 'void-function)
;; frameset-p only exists after (require 'frameset) — see last section.
(qa-err (frameset-p nil) 'void-function)

;; == window-state-get: sole leaf window ==
(let ((qaws-st (window-state-get)))
  ;; element 0 is the min-size alist, element 1 the type tag
  (qa (consp (car qaws-st)) t)
  (qa (cdr (assq 'min-height (car qaws-st))) 4)
  (qa (cdr (assq 'min-width (car qaws-st))) 10)
  (qa (cdr (assq 'min-height-safe (car qaws-st))) 1)
  (qa (cdr (assq 'min-width-safe (car qaws-st))) 2)
  (qa (cdr (assq 'min-height-ignore (car qaws-st))) 2)
  (qa (cdr (assq 'min-pixel-height-safe (car qaws-st))) 1)
  (qa (nth 1 qaws-st) 'leaf)
  (qa (length qaws-st) 10)
  (qa (cdr (assq 'pixel-width qaws-st)) 80)
  (qa (cdr (assq 'pixel-height qaws-st)) 24)
  (qa (cdr (assq 'total-width qaws-st)) 80)
  (qa (cdr (assq 'total-height qaws-st)) 24)
  (qa (cdr (assq 'normal-height qaws-st)) 1.0)
  (qa (cdr (assq 'normal-width qaws-st)) 1.0)
  ;; parameters slot holds a clone-of back-reference to the window
  (qa (car (cadr (assq 'parameters qaws-st))) 'clone-of)
  ;; buffer slot: (BUFFER selected hscroll fringes margins scroll-bars
  ;; vscroll dedicated point start) — objects, not printable values
  (let ((qaws-bs (assq 'buffer qaws-st)))
    (qa (buffer-name (nth 1 qaws-bs)) "*scratch*")
    (qa (cdr (assq 'selected (cddr qaws-bs))) t)
    (qa (cdr (assq 'hscroll (cddr qaws-bs))) 0)
    (qa (equal (cdr (assq 'fringes (cddr qaws-bs))) '(0 0 nil nil)) t)
    (qa (equal (cdr (assq 'margins (cddr qaws-bs))) '(nil)) t)
    (qa (equal (cdr (assq 'scroll-bars (cddr qaws-bs)))
               '(nil 0 t nil 0 t nil)) t)
    (qa (cdr (assq 'vscroll (cddr qaws-bs))) 0)
    (qa (equal (assq 'dedicated (cddr qaws-bs)) '(dedicated)) t)
    (qa (markerp (cdr (assq 'point (cddr qaws-bs)))) t)
    (qa (markerp (cdr (assq 'start (cddr qaws-bs)))) t)))

;; == window-state-get WRITABLE=t: printable serialization ==
(let ((qaws-st (window-state-get nil t)))
  (qa (nth 1 qaws-st) 'leaf)
  ;; writable drops the unreadable `parameters' slot → 9 elements
  (qa (length qaws-st) 9)
  (qa (assq 'parameters qaws-st) nil)
  (let ((qaws-bs (assq 'buffer qaws-st)))
    (qa (nth 1 qaws-bs) "*scratch*")          ; buffer name string
    (qa (cdr (assq 'point (cddr qaws-bs))) 1) ; integer, not marker
    (qa (cdr (assq 'selected (cddr qaws-bs))) t)))
;; each call builds a fresh list
(qa (eq (window-state-get) (window-state-get)) nil)

;; == window-state-get on the minibuffer window ==
(qa (nth 1 (window-state-get (minibuffer-window))) 'leaf)
(qa (nth 1 (window-state-get (minibuffer-window) t)) 'leaf)
;; a frame is not a window for state-get
(qa-err (window-state-get (selected-frame)) 'error)
(qa-err (window-state-get 'qaws-x) 'error)     ; "not a live or internal window"

;; == window-state-put: leaf roundtrip on the sole window ==
;; NB: a non-writable state carries (parameters (clone-of . WINDOW));
;; putting it *replaces* that window, which perturbs later splits.
;; Roundtrip via writable states to keep the tree clean.
(let ((qaws-st (window-state-get nil t)))
  (qa (window-state-put qaws-st) nil)
  (let ((qaws-st2 (window-state-get)))
    (qa (nth 1 qaws-st2) 'leaf)
    (qa (cdr (assq 'total-width qaws-st2)) 80)
    (qa (cdr (assq 'total-height qaws-st2)) 24)
    (qa (cdr (assq 'normal-height qaws-st2)) 1.0)))
(let ((qaws-st (window-state-get nil t)))
  (qa (window-state-put qaws-st) nil)
  (qa (nth 1 (window-state-get)) 'leaf))
;; SAFE arg accepted
(qa (window-state-put (window-state-get nil t) nil t) nil)

;; == window-state-put / -get argument errors ==
;; GNU checks arg types in a surprising order: a bogus symbol hits
;; number-or-marker-p, not listp — pin only the error class.
(qa-err (window-state-put 'qaws-bogus) 'wrong-type-argument)
(qa-err (window-state-put nil) 'wrong-type-argument)
(qa-err (window-state-put '(qaws-foo)) 'wrong-type-argument)
(qa-err (window-state-put (window-state-get) 'qaws-x) 'error) ; not a window

;; == window sizes: total/body/min/normal ==
(qa (window-total-size) 24)                    ; height in lines
(qa (window-total-size nil t) 80)              ; width in columns
(qa (window-total-size (minibuffer-window)) 1)
(qa (window-total-size (minibuffer-window) t) 80)
(qa (window-body-size) 23)
(qa (window-body-size nil nil t) 23)           ; pixelwise: 1px chars
(qa (window-body-size (minibuffer-window)) 1)
(qa (window-min-size) 4)
(qa (window-min-size nil t) 10)
(qa (window-min-size nil nil t) 2)
(qa (window-min-size nil t t) 2)
(qa (window-min-size (minibuffer-window)) 4)
(qa (window-normal-size) 1.0)
(qa (window-normal-size nil t) 1.0)
(qa (window-max-chars-per-line) 79)
(qa (window-max-chars-per-line (minibuffer-window)) 79)
(qa (window-buffer-height (selected-window)) 1)
(qa-err (window-buffer-height) 'wrong-number-of-arguments)

;; == pixel-measuring fns: all degenerate in batch ==
(qa (window-text-pixel-size) '(0 . 0))         ; cons W . H
(with-temp-buffer
  (insert "qaws-hello\nqaws-world!\n")
  ;; BUFFER arg accepted; still (0 . 0) — nothing was ever displayed
  (qa (window-text-pixel-size nil nil nil nil nil (current-buffer))
      '(0 . 0)))
(qa (natnump (car (window-text-pixel-size))) t)
(qa (natnump (cdr (window-text-pixel-size))) t)
(qa (window-lines-pixel-dimensions) nil)       ; needs real display
(qa (window-lines-pixel-dimensions (minibuffer-window)) nil)
(qa (window-text-pixel-size (minibuffer-window)) '(0 . 0))
(qa (window-old-pixel-width) 0)
(qa (window-old-pixel-height) 0)
(qa (window-pixel-width-before-size-change) 0)
(qa (window-pixel-height-before-size-change) 0)
(qa (window-font-width) 1)
(qa (window-font-height) 1)
(qa (window-text-width) 80)

;; == sizable/resizable/resize on the sole (root) window ==
(qa (window-sizable-p (selected-window) 1) t)
(qa (window-sizable-p (selected-window) 1 t) t)
(qa (window-sizable-p (selected-window) -1) t)
(qa (window-sizable-p (selected-window) 0) t)
;; window-resizable returns the available DELTA, not a boolean
(qa (window-resizable (selected-window) 1) 0)
(qa (window-resizable (selected-window) -1 t) 0)
(qa-err (window-sizable-p (selected-window)) 'wrong-number-of-arguments)
(qa-err (window-resize (selected-window) 1) 'error)   ; can't resize root
(qa-err (window-resize (selected-window) 1 t) 'error)
(qa-err (window-resize (selected-window) 1 nil t) 'error)
(qa-err (window-resize 'qaws-x 1) 'error)
(qa (fit-window-to-buffer) nil)

;; == side windows: none exist in batch; probe via parameters ==
(qa (window-parameter nil 'window-side) nil)
(qa (window-parameter nil 'window-slot) nil)
(qa (window-parameter nil 'quit-restore) nil)
(qa (window-parameter nil 'min-margins) nil)
(qa-err (window-toggle-side-windows) 'error)   ; "No side windows state found"

;; == window-in-direction ==
(qa (window-in-direction 'above) nil)
(qa (window-in-direction 'below) nil)          ; minibuffer is skipped
(qa (window-in-direction 'left) nil)
(qa (window-in-direction 'right) nil)
(qa (window-in-direction 'up) nil)
(qa (window-in-direction 'down) nil)
(qa (window-in-direction 'above nil nil nil t) nil) ; WRAP still finds none
(qa-err (window-in-direction 'qaws-diagonal) 'error) ; "Wrong direction"

;; == split: tree accessors, edges, state shape ==
(let* ((qaws-w1 (selected-window))
       (qaws-w2 (split-window)))
  ;; TTY quirk: top window occupies rows 1-12, bottom 13-24
  (qa (window-edges qaws-w1) '(0 1 80 13))
  (qa (window-edges qaws-w2) '(0 13 80 25))
  (qa (window-inside-edges qaws-w1) '(0 0 80 11))
  (qa (not (equal (window-edges qaws-w1) (window-inside-edges qaws-w1))) t)
  (qa (window-total-height qaws-w2) 12)
  (qa (window-total-size qaws-w2) 12)
  (qa (window-total-size qaws-w2 t) 80)
  (qa (window-body-size qaws-w2) 11)
  (qa (window-min-size qaws-w2) 4)
  (qa (window-min-size qaws-w2 t) 10)
  ;; GNU builds a real vc tree
  (qa (nth 1 (window-state-get)) 'vc)
  (qa (length (window-state-get)) 12)
  ;; header of the internal node has doubled height bounds
  (qa (cdr (assq 'min-height (car (window-state-get)))) 8)
  (qa (cdr (assq 'min-height-safe (car (window-state-get)))) 2)
  ;; children are the two trailing elements, each (leaf . SLOTS) —
  ;; child states carry NO min-size header; the last child's second
  ;; element is a (last . t) marker
  (qa (car (nth 9 (window-state-get nil t))) 'leaf)
  (qa (car (nth 10 (window-state-get nil t))) 'leaf)
  (qa (nth 1 (nth 10 (window-state-get nil t))) '(last . t))
  ;; child window serializes as its own leaf state
  (qa (nth 1 (window-state-get qaws-w2)) 'leaf)
  (qa (cdr (assq 'total-height (window-state-get qaws-w2))) 12)
  ;; parent / children
  (qa (windowp (window-parent qaws-w2)) t)
  (qa-same (window-parent qaws-w2) (window-parent qaws-w1))
  (qa (window-live-p (window-parent qaws-w1)) nil)
  (qa-same (nth 1 (window-state-get (window-parent qaws-w2))) 'vc)
  (qa-same (window-top-child (window-parent qaws-w1)) qaws-w1)
  (qa (window-left-child (window-parent qaws-w1)) nil)
  ;; combination-limit defaults to nil on internal windows
  (qa (window-combination-limit (window-parent qaws-w2)) nil)
  (qa (set-window-combination-limit (window-parent qaws-w2) t) t)
  (qa (window-combination-limit (window-parent qaws-w2)) t)
  (qa-err (window-combination-limit (selected-window)) 'error) ; leaf → error
  ;; window-combined-p returns the TOP/LEFT window of the combination —
  ;; for the first child that is the window itself
  (qa-same (window-combined-p qaws-w2) qaws-w1)
  (qa-same (window-combined-p qaws-w1) qaws-w1)
  (qa (window-combined-p qaws-w2 t) nil)       ; not horizontally combined
  ;; siblings
  (qa-same (window-next-sibling qaws-w1) qaws-w2)
  (qa-same (window-prev-sibling qaws-w2) qaws-w1)
  ;; cyclic navigation
  (qa-same (next-window qaws-w1) qaws-w2)
  (qa-same (next-window qaws-w2) qaws-w1)
  (qa-same (previous-window qaws-w1) qaws-w2)
  (qa-same (previous-window qaws-w2) qaws-w1)
  ;; other-window batch quirk: returns nil but SELECTS the other
  ;; window — subsequent (window-in-direction …) then starts there
  (qa (other-window 1) nil)
  (qa-same (selected-window) qaws-w2)
  (select-window qaws-w1)
  ;; in-direction now finds the sibling
  (qa-same (window-in-direction 'below) qaws-w2)
  (qa-same (window-in-direction 'down) qaws-w2)
  (qa-same (window-in-direction 'above qaws-w2) qaws-w1)
  (qa (window-in-direction 'above) nil)
  ;; window-tree: (VERTICAL EDGES . WINDOWS) + minibuffer
  (let ((qaws-wt (window-tree)))
    (qa (eq (car (car qaws-wt)) t) t)
    (qa (nth 1 (car qaws-wt)) '(0 1 80 25))
    (qa (eq (cadr qaws-wt) (minibuffer-window)) t))
  ;; walk-windows visits live windows only (no minibuffer by default)
  (let ((qaws-n 0))
    (walk-windows (lambda (qaws-w) (when (windowp qaws-w) (setq qaws-n (1+ qaws-n)))))
    (qa qaws-n 2))
  ;; resizable now reports real deltas
  (qa (window-resizable qaws-w2 5) 5)
  (qa (window-resizable qaws-w2 -5) -5)
  (qa (window-resizable qaws-w1 1) 1)
  (qa (window-sizable-p qaws-w2 -5) t)
  (qa (window-sizable-p qaws-w2 5) t)
  ;; resizing by lines
  (qa (window-resize qaws-w2 2) t)
  (qa (window-total-height qaws-w2) 14)
  (qa (window-total-height qaws-w1) 10)
  (qa-err (window-resize qaws-w2 100) 'error)
  ;; window-at: row 12 already hits w2's area (w1's bottom edge is its
  ;; mode line)
  (qa-same (window-at 0 1) qaws-w1)
  (qa-same (window-at 0 12) qaws-w2)
  (qa-same (window-at 0 20) qaws-w2)
  ;; delete: object stays a window but stops being valid/live
  (qa (window-valid-p qaws-w2) t)
  (qa (delete-window qaws-w2) nil)
  (qa (window-valid-p qaws-w2) nil)
  (qa (windowp qaws-w2) t)
  (qa (window-live-p qaws-w2) nil))

;; == state-put restoring a saved vc tree ==
(let* ((qaws-w2 (split-window))
       (qaws-st (window-state-get nil t)))    ; writable: no clone-of
  (delete-window qaws-w2)
  (qa (count-windows) 1)
  (qa (window-state-put qaws-st) nil)
  (qa (count-windows) 2)
  (qa (nth 1 (window-state-get)) 'vc)
  (delete-other-windows))
;; state ops on a dead window error
(let ((qaws-w2 (split-window)))
  (delete-window qaws-w2)
  (qa-err (window-state-get qaws-w2) 'error)
  (qa-err (window-state-put (window-state-get) qaws-w2) 'error))

;; == horizontal split ==
(let* ((qaws-w1 (selected-window))
       (qaws-w2 (split-window nil nil t)))
  (qa (window-edges qaws-w1) '(0 1 40 25))
  (qa (window-edges qaws-w2) '(40 1 80 25))
  (qa (nth 1 (window-state-get)) 'hc)
  (qa-same (window-combined-p qaws-w2 t) qaws-w1)
  (qa (window-combined-p qaws-w2) nil)         ; not vertically combined
  (qa-same (window-in-direction 'right) qaws-w2)
  (qa (window-in-direction 'left) nil)
  (qa-same (window-left-child (window-parent qaws-w1)) qaws-w1)
  (qa (window-total-size qaws-w2 t) 40)
  (qa (window-total-size qaws-w2) 24)
  (qa (window-max-chars-per-line qaws-w2) 39)
  (qa-same (next-window qaws-w1) qaws-w2)
  (qa (delete-window qaws-w2) nil))

;; == window-main-window / atom root ==
(qa-same (window-main-window) (frame-root-window))
(qa-same (frame-first-window) (frame-root-window))
(let ((qaws-w2 (split-window)))
  ;; GNU: main-window is the internal parent window after a split
  (qa-same (window-main-window) (window-parent qaws-w2))
  ;; no atoms configured → atom-root is nil
  (qa (window-atom-root) nil)
  (qa (window-atom-root qaws-w2) nil)
  (delete-window qaws-w2))

;; == misc fns on real windows ==
(qa (fboundp 'run-window-configuration-change-hook) t)
(qa (fboundp 'window-old-pixel-width) t)
(qa (fboundp 'window-font-width) t)
(qa window-size-change-functions nil)
(qa split-window-keep-point t)
(qa (window-no-other-p) nil)
(qa (delete-other-windows-vertically (selected-window)) nil) ; nothing above/below
(let ((qaws-w2 (split-window)))
  ;; returns the list of deleted windows
  (qa (listp (delete-other-windows-vertically)) t)
  (qa (count-windows) 1)
  (when (> (count-windows) 1) (delete-other-windows)))
;; replace-buffer-in-windows swaps a displayed buffer out
(let ((qaws-b (get-buffer-create " qaws-rbw"))
      (qaws-w2 (split-window)))
  (set-window-buffer qaws-w2 qaws-b)
  (qa-same (window-buffer qaws-w2) qaws-b)
  (qa (replace-buffer-in-windows qaws-b) nil)
  (qa (eq (window-buffer qaws-w2) qaws-b) nil)
  (qa (delete-window qaws-w2) nil)
  (kill-buffer qaws-b))
;; display-buffer with a reuse action reuses the live window
(with-temp-buffer
  (rename-buffer " qaws-db")
  (qa (windowp (display-buffer (current-buffer)
                               '((display-buffer-reuse-window)))) t))
;; minibuffer window identity/predicate (frameset needs these stable)
(qa-same (window-frame (minibuffer-window)) (selected-frame))
(qa (window-minibuffer-p (minibuffer-window)) t)
(qa (window-minibuffer-p (selected-window)) nil)
;; window-side is only a *parameter*; setting it manually doesn't make a
;; real side window (toggle still fails), but the flag round-trips.
(let ((qaws-w2 (split-window)))
  (set-window-parameter qaws-w2 'window-side 'bottom)
  (qa (window-parameter qaws-w2 'window-side) 'bottom)
  (set-window-parameter qaws-w2 'window-slot 2)
  (qa (window-parameter qaws-w2 'window-slot) 2)
  (qa (window-parameter nil 'window-side) nil) ; selected window unaffected
  (delete-window qaws-w2))

;; == frameset (requires frameset.el) ==
(qa (require 'frameset) 'frameset)
(let ((qaws-fs (frameset-save nil)))
  (qa (vectorp qaws-fs) t)
  (qa (length qaws-fs) 8)
  (qa (aref qaws-fs 0) 'frameset)
  (qa (aref qaws-fs 1) 1)                      ; record version
  (qa (frameset-p qaws-fs) t)
  (qa (frameset-p nil) nil)
  (qa (frameset-p "qaws") nil)
  (qa (frameset-p [1 2 3]) nil)
  ;; :reuse-frames nil wants to build frames — silently a no-op in batch
  (qa (frameset-restore qaws-fs :reuse-frames nil) nil)
  (qa (length (frame-list)) 1))
(qa (vectorp (frameset-save (frame-list))) t)
(qa-err (frameset-save 'qaws-x) 'wrong-type-argument)
(qa-err (frameset-restore 'qaws-x) 'cl-assertion-failed)

(princ (qa--done))
