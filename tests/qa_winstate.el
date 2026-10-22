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

;; Issue #285: persistent state parameters use independent alist associations.
(defun qa285-fixture (function)
  (save-window-excursion
    (delete-other-windows)
    (let* ((leaf (split-window nil nil 'right))
           (root (frame-root-window)))
      (funcall function leaf root))))
(defun qa285-get (flag writable internal)
  (qa285-fixture
   (lambda (leaf root)
     (let* ((key (make-symbol "probe"))
            (window (if internal root leaf))
            (window-persistent-parameters (list (cons key flag))))
       (set-window-parameter window key 1)
       (cdr (assq key (cdr (assq 'parameters (window-state-get window writable)))))))))
(defun qa285-clone (flag writable present)
  (qa285-fixture
   (lambda (leaf _root)
     (let ((window-persistent-parameters (list (cons 'clone-of flag))))
       (when present (set-window-parameter leaf 'clone-of 'original))
       (let* ((state (window-state-get leaf writable))
              (pair (assq 'clone-of (cdr (assq 'parameters state)))))
         (list (not (null pair)) (eq (cdr pair) leaf) (eq (cdr pair) 'original)))))))
(defun qa285-put (kind)
  (qa285-fixture
   (lambda (window _root)
     (let* ((window-persistent-parameters nil)
            (key (make-symbol "saved")) (other (make-symbol "other"))
            (state (window-state-get window t)))
       (set-window-parameter window key 2)
       (set-window-parameter window other 9)
       (when kind (setq state (append state (list (cons 'parameters (list (cons key (if (eq kind 'nil-value) nil 1))))))))
       (window-state-put state window)
       (list (window-parameter window key) (window-parameter window other)
             (not (null (assq other (window-parameters window)))))))))

(qa
(list (qa285-get 'nil nil nil) (qa285-get 'nil t nil))
'(nil nil)
)

(qa
(list (qa285-get 't nil nil) (qa285-get 't t nil))
'(1 nil)
)

(qa
(list (qa285-get 'writable nil nil) (qa285-get 'writable t nil))
'(1 1)
)

(qa
(list (qa285-get 'other nil nil) (qa285-get 'other t nil))
'(1 nil)
)

(qa
(list (qa285-get 'nil nil t) (qa285-get 'nil t t))
'(nil nil)
)

(qa
(list (qa285-get 't nil t) (qa285-get 't t t))
'(1 nil)
)

(qa
(list (qa285-get 'writable nil t) (qa285-get 'writable t t))
'(1 1)
)

(qa
(list (qa285-get 'other nil t) (qa285-get 'other t t))
'(1 nil)
)

(qa
(list (qa285-clone 'nil nil nil) (qa285-clone 'nil nil t) (qa285-clone 'nil t nil) (qa285-clone 'nil t t))
'((nil nil nil) (nil nil nil) (nil nil nil) (nil nil nil))
)

(qa
(list (qa285-clone 't nil nil) (qa285-clone 't nil t) (qa285-clone 't t nil) (qa285-clone 't t t))
'((t t nil) (t nil t) (nil nil nil) (nil nil nil))
)

(qa
(list (qa285-clone 'writable nil nil) (qa285-clone 'writable nil t) (qa285-clone 'writable t nil) (qa285-clone 'writable t t))
'((t t nil) (t nil t) (t t nil) (t nil t))
)

(qa
(list (qa285-clone 'other nil nil) (qa285-clone 'other nil t) (qa285-clone 'other t nil) (qa285-clone 'other t t))
'((t t nil) (t nil t) (nil nil nil) (nil nil nil))
)

(qa
(qa285-put 'nil)
'(nil nil t)
)

(qa
(qa285-put 'value)
'(1 nil t)
)

(qa
(qa285-put 'nil-value)
'(nil nil t)
)

(qa
(qa285-fixture
 (lambda (window _root)
  (let* ((key (make-symbol "copy")) (value (list 1))
         (window-persistent-parameters (list (cons key 'writable))))
   (set-window-parameter window key value)
   (let* ((state (window-state-get window t))
          (pair (assq key (cdr (assq 'parameters state)))))
    (set-window-parameter window key 'changed)
    (setcar value 9)
    (list (eq (cdr pair) value) (cdr pair)
          (progn (setcdr pair 'snapshot) (window-parameter window key)))))))
'(t (9) changed)
)

(qa
(qa285-fixture
 (lambda (window _root)
  (let* ((key (make-symbol "nil"))
         (window-persistent-parameters (list (cons key 'writable))))
   (set-window-parameter window key nil)
   (let ((pair (assq key (cdr (assq 'parameters (window-state-get window t))))))
    (list (not (null pair)) (cdr pair))))))
'(t nil)
)

(qa
(qa285-fixture
 (lambda (leaf root)
  (let* ((key 'qa285-tree-key)
         (window-persistent-parameters (list (cons key 'writable))))
   (set-window-parameter root key 11)
   (set-window-parameter leaf key 22)
   (let ((state (window-state-get root t)))
    (delete-other-windows)
    (window-state-put (read (prin1-to-string state)) (selected-window))
    (let* ((new-root (frame-root-window))
           (first (window-child new-root))
           (second (window-next-sibling first)))
     (list (window-parameter new-root key)
           (window-parameter first key)
           (window-parameter second key)))))))
'(11 nil 22)
)

(qa
(qa285-fixture
 (lambda (window _root)
  (let ((window-persistent-parameters '((clone-of . writable))))
   (set-window-parameter window 'clone-of nil)
   (let ((pair (assq 'clone-of (cdr (assq 'parameters (window-state-get window t))))))
    (list (not (null pair)) (cdr pair))))))
'(t nil)
)

(qa
(qa285-fixture
 (lambda (window _root)
  (let* ((key (make-symbol "restore-copy")) (value (list 1))
         (window-persistent-parameters nil)
         (state (window-state-get window t)) (pair (cons key value)))
   (setq state (append state (list (cons 'parameters (list pair)))))
   (window-state-put state window)
   (setcdr pair 'snapshot)
   (setcar value 9)
   (list (eq (window-parameter window key) value)
         (window-parameter window key)))))
'(t (9))
)

;; Issue #286: normal dimensions follow the immediate parent combination.
;; Derive expected fractions from local dimensions: batch frame heights differ.
(defun qa286-fixture (function)
  (save-window-excursion
    (delete-other-windows)
    (funcall function (selected-window))))
(defun qa286-normal (window)
  (list (window-normal-size window) (window-normal-size window t)))
(defun qa286-split (side size)
  (qa286-fixture
   (lambda (old)
     (let* ((horizontal (memq side '(left right)))
            (total (window-total-size old horizontal))
            (expected-new (if (null size) 0.5
                            (/ (float (if (< size 0) (- size) (- total size))) total)))
            (expected-old (- 1.0 expected-new))
            (new (split-window old size side)))
       (list (equal (qa286-normal old)
                    (if horizontal (list 1.0 expected-old) (list expected-old 1.0)))
             (equal (qa286-normal new)
                    (if horizontal (list 1.0 expected-new) (list expected-new 1.0)))
             (qa286-normal (window-parent new)))))))

(qa
(qa286-split 'right 20)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'left 20)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'below 10)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'above 10)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'right nil)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'below nil)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'right -20)
'(t t (1.0 1.0))
)

(qa
(qa286-split 'below -10)
'(t t (1.0 1.0))
)

(qa
(qa286-fixture
 (lambda (left)
  (let* ((right (split-window left 20 'right))
         (total (window-total-height right))
         (expected-new (/ (float (- total 10)) total))
         (bottom (split-window right 10 'below)))
   (list (qa286-normal left)
         (equal (qa286-normal right) (list (- 1.0 expected-new) 1.0))
         (equal (qa286-normal bottom) (list expected-new 1.0))
         (qa286-normal (window-parent bottom)) (qa286-normal (frame-root-window))))))
'((1.0 0.25) t t (1.0 0.75) (1.0 1.0))
)

(qa
(qa286-fixture
 (lambda (left)
  (let* ((middle (split-window left 20 'right))
         (right (split-window middle 30 'right)))
   (mapcar #'qa286-normal (list left middle right (frame-root-window))))))
'((1.0 0.25) (1.0 0.375) (1.0 0.375) (1.0 1.0))
)

(qa
(qa286-fixture
 (lambda (left)
  (let* ((right (split-window left 20 'right))
         (saved (current-window-configuration)))
   (split-window right 10 'below)
   (set-window-configuration saved)
   (list (qa286-normal left) (qa286-normal right)))))
'((1.0 0.25) (1.0 0.75))
)

(qa
(qa286-fixture
 (lambda (left)
  (let ((right (split-window left 20 'right)))
   (set-window-new-normal right 0.9)
   (list (qa286-normal left) (qa286-normal right) (window-new-normal right)))))
'((1.0 0.25) (1.0 0.75) 0.9)
)

(qa
(list (qa286-normal (frame-root-window)) (qa286-normal (minibuffer-window)))
'((1.0 1.0) (1.0 1.0))
)

(qa
(qa286-fixture
 (lambda (left)
  (let* ((middle (split-window left 20 'right))
         (right (split-window middle 30 'right)))
   (delete-window middle)
   (list (qa286-normal left) (qa286-normal right)
         (progn (delete-window left) (qa286-normal right))))))
'((1.0 0.625) (1.0 0.375) (1.0 1.0))
)

(qa
(qa286-fixture
 (lambda (left)
  (let ((right (split-window left 20 'right)))
   (delete-other-windows right) (qa286-normal right))))
'(1.0 1.0)
)

(qa
(qa286-fixture
 (lambda (left)
  (let ((right (split-window left 20 'right)))
   (enlarge-window 5 t)
   (list (qa286-normal left) (qa286-normal right)))))
'((1.0 0.3125) (1.0 0.6875))
)

(qa
(qa286-fixture
 (lambda (top)
  (split-window top nil 'below)
  (let ((state (window-state-get nil t)))
   (delete-other-windows)
   (window-state-put (read (prin1-to-string state)))
   (let ((root (frame-root-window)))
    (list (mapcar (lambda (window)
                    (list (= (window-normal-size window)
                             (/ (float (window-total-height window))
                                (window-total-height root)))
                          (window-normal-size window t)))
                  (window-list))
          (= 1.0 (apply #'+ (mapcar #'window-normal-size (window-list)))))))))
'(((t 1.0) (t 1.0)) t)
)

(qa
(qa286-fixture
 (lambda (top)
  (let* ((bottom (split-window top nil 'below))
         (saved (current-window-configuration)))
   (enlarge-window 1)
   (set-window-configuration saved)
   (list (qa286-normal top) (qa286-normal bottom)))))
'((0.5 1.0) (0.5 1.0))
)

;; Issue #287: use-time bumps preserve the selected window as most recent.
(defun qa287-fixture (function)
  (save-window-excursion
    (delete-other-windows)
    (let* ((first (selected-window))
           (second (split-window first 20 'right))
           (third (split-window second 20 'right)))
      (select-window first) (select-window second) (select-window third)
      (funcall function first second third))))
(defun qa287-times (windows) (mapcar #'window-use-time windows))

(qa
(qa287-fixture
 (lambda (first second third)
  (let* ((before (window-use-time third)) (result (window-bump-use-time first)))
   (list (= result before) (= (window-use-time first) before)
         (= (window-use-time third) (1+ before))
         (> (window-use-time first) (window-use-time second))
         (eq (selected-window) third)))))
'(t t t t t)
)

(qa
(qa287-fixture
 (lambda (_first _second third)
  (let ((before (window-use-time third)))
   (list (window-bump-use-time third) (= before (window-use-time third))
         (window-bump-use-time nil) (= before (window-use-time third))))))
'(nil t nil t)
)

(qa
(qa287-fixture
 (lambda (first second third)
  (select-window first 'norecord)
  (let ((before (qa287-times (list first second third))))
   (list (window-bump-use-time second)
         (equal before (qa287-times (list first second third)))
         (eq (selected-window) first)))))
'(nil t t)
)

(qa
(qa287-fixture
 (lambda (first second third)
  (select-window first 'norecord)
  (delete-window third)
  (let ((before (qa287-times (list first second))))
   (list (window-bump-use-time second)
         (equal before (qa287-times (list first second)))))))
'(nil t)
)

(qa
(qa287-fixture
 (lambda (first _second third)
  (let* ((before (window-use-time third))
         (first-result (window-bump-use-time first))
         (second-result (window-bump-use-time first)))
   (list (= first-result before) (= second-result (1+ before))
         (= (window-use-time third) (+ before 2))
         (eq (selected-window) third)))))
'(t t t t)
)

(qa
(qa287-fixture
 (lambda (_first _second third)
  (let* ((mini (minibuffer-window)) (before (window-use-time third))
         (result (window-bump-use-time mini)))
   (list (= result before) (= (window-use-time mini) before)
         (= (window-use-time third) (1+ before))
         (eq (selected-window) third)))))
'(t t t t)
)

(qa
(condition-case err (window-bump-use-time 17)
 (error (list (car err) (cadr err))))
'(wrong-type-argument window-live-p)
)

(qa
(qa287-fixture
 (lambda (_first _second _third)
  (condition-case err (window-bump-use-time (frame-root-window))
   (error (list (car err) (cadr err))))))
'(wrong-type-argument window-live-p)
)

(qa
(qa287-fixture
 (lambda (first _second third)
  (select-window first) (delete-window third)
  (condition-case err (window-bump-use-time third)
   (error (list (car err) (cadr err))))))
'(wrong-type-argument window-live-p)
)

(qa
(qa287-fixture
 (lambda (first _second third)
  (let* ((before (window-use-time third))
         (result (window--display-buffer (window-buffer first) first 'reuse '((bump-use-time . t)))))
   (list (eq result first) (= (window-use-time first) before)
         (= (window-use-time third) (1+ before)) (eq (selected-window) third)))))
'(t t t t)
)

(qa
(qa287-fixture
 (lambda (first second third)
  (select-window first 'norecord)
  (let ((before (qa287-times (list first second third))))
   (window--display-buffer (window-buffer second) second 'reuse '((bump-use-time . t)))
   (equal before (qa287-times (list first second third))))))
't
)

(qa
(qa287-fixture
 (lambda (first second third)
  (let ((original (symbol-function 'window-bump-use-time)) called
        (before (qa287-times (list first second third))))
   (unwind-protect
       (progn
         (fset 'window-bump-use-time (lambda (window) (setq called window)))
         (window--display-buffer (window-buffer first) first 'reuse '((bump-use-time . t)))
         (list (eq called first) (equal before (qa287-times (list first second third)))))
     (fset 'window-bump-use-time original)))))
'(t t)
)

;; Issue #288: past window point is an independent marker.
(defun qa288-fixture (position function)
 (save-window-excursion
  (delete-other-windows)
  (with-temp-buffer
   (switch-to-buffer (current-buffer))
   (insert "012345678901234567890123456789\n") (goto-char position)
   (funcall function (selected-window)))))


(qa
(qa288-fixture 1 (lambda (first) (let ((other (split-window first 20 'right))) (list (window-point other) (window-old-point other) (window-old-point first)))))
'(1 1 1)
)

(qa
(qa288-fixture 5 (lambda (first) (let ((other (split-window first 20 'right))) (list (window-point other) (window-old-point other) (window-old-point first)))))
'(5 5 1)
)

(qa
(qa288-fixture 10 (lambda (first) (let ((other (split-window first 20 'right))) (list (window-point other) (window-old-point other) (window-old-point first)))))
'(10 10 1)
)

(qa
(qa288-fixture 1 (lambda (first) (let ((other (split-window first 20 'right))) (set-window-point other 10) (list (window-point other) (window-old-point other)))))
'(10 1)
)

(qa
(qa288-fixture 1 (lambda (first) (let ((other (split-window first 20 'right))) (set-window-point other 10) (select-window other) (list (window-point other) (window-old-point other)))))
'(10 1)
)

(qa
(qa288-fixture 1 (lambda (_first) (goto-char 10) (list (point) (window-old-point))))
'(10 1)
)

(qa
(qa288-fixture 5 (lambda (first) (let* ((window-point-insertion-type nil) (other (split-window first 20 'right))) (goto-char 1) (insert "abc") (let ((before (window-old-point other))) (goto-char 8) (insert "xyz") (list before (window-old-point other))))))
'(8 8)
)

(qa
(qa288-fixture 5 (lambda (first) (let* ((window-point-insertion-type t) (other (split-window first 20 'right))) (goto-char 1) (insert "abc") (let ((before (window-old-point other))) (goto-char 8) (insert "xyz") (list before (window-old-point other))))))
'(8 11)
)

(qa
(qa288-fixture 10 (lambda (first) (let ((other (split-window first 20 'right))) (delete-region 1 6) (let ((before (window-old-point other))) (delete-region 1 8) (list before (window-old-point other))))))
'(5 1)
)

(qa
(qa288-fixture 5 (lambda (first) (let ((other (split-window first 20 'right))) (narrow-to-region 10 20) (list (point-min) (window-old-point other)))))
'(10 5)
)

(qa
(qa288-fixture 5 (lambda (first) (let ((other (split-window first 20 'right))) (garbage-collect) (goto-char 1) (insert "abc") (window-old-point other))))
'8
)

(qa
(qa288-fixture 5 (lambda (first) (let* ((other (split-window first 20 'right)) (saved (current-window-configuration))) (set-window-point other 10) (goto-char 1) (insert "abc") (set-window-configuration saved) (let ((before (window-old-point other))) (goto-char 1) (insert "xyz") (set-window-configuration saved) (list before (window-old-point other))))))
'(8 11)
)

(qa
(qa288-fixture 1 (lambda (first) (let ((other (split-window first 20 'right))) (set-window-point other 10) (set-window-buffer other (window-buffer first)) (list (window-point other) (window-old-point other)))))
'(1 1)
)

(qa
(qa288-fixture 1 (lambda (first) (let ((other (split-window first 20 'right))) (with-temp-buffer (insert "abcdefghij") (goto-char 7) (set-window-buffer other (current-buffer)) (let ((before (window-old-point other))) (goto-char 1) (insert "abc") (list before (window-old-point other)))))))
'(7 10)
)

(qa
(condition-case err (window-old-point 42) (error (list (car err) (cadr err))))
'(wrong-type-argument window-live-p)
)

(qa
(qa288-fixture 1 (lambda (first) (split-window first 20 'right) (condition-case err (window-old-point (frame-root-window)) (error (list (car err) (cadr err))))))
'(wrong-type-argument window-live-p)
)

(qa
(qa288-fixture 1 (lambda (first) (let ((other (split-window first 20 'right))) (delete-window other) (condition-case err (window-old-point other) (error (list (car err) (cadr err)))))))
'(wrong-type-argument window-live-p)
)

(qa
(qa288-fixture 5
 (lambda (first)
  (let* ((other (split-window first 20 'right))
         (saved (current-window-configuration)))
   (goto-char 7) (set-window-buffer other (window-buffer first))
   (set-window-configuration saved)
   (let ((before (window-old-point other)))
    (goto-char 9) (set-window-buffer other (window-buffer first))
    (set-window-configuration saved)
    (list before (window-old-point other))))))
'(5 5)
)

;; Issue #289: old buffer changes only at notification or deletion boundaries.
(defun qa289-fixture (function)
 (save-window-excursion
  (delete-other-windows)
  (let ((a (generate-new-buffer " *qa289-a*"))
        (b (generate-new-buffer " *qa289-b*")))
   (unwind-protect (funcall function (selected-window) a b)
    (kill-buffer a) (kill-buffer b)))))

(qa
(qa289-fixture (lambda (window a b)
 (let ((old (window-old-buffer window)) (previous (window-buffer window)))
  (set-window-buffer window a) (set-window-buffer window b)
  (list (eq (window-old-buffer window) old) (eq (window-old-buffer window) previous)))))
'(t nil)
)

(qa
(qa289-fixture (lambda (window a b)
 (let ((other (split-window window 20 'right)))
  (set-window-buffer other a) (set-window-buffer other b) (window-old-buffer other))))
'nil
)

(qa
(qa289-fixture (lambda (window _a _b)
 (let ((old (window-old-buffer window)))
  (set-window-buffer window (window-buffer window))
  (eq old (window-old-buffer window)))))
't
)

(qa
(qa289-fixture (lambda (window a b)
 (let* ((other (split-window window 20 'right)))
  (set-window-buffer other a)
  (let ((saved (current-window-configuration)))
   (set-window-buffer other b) (set-window-configuration saved)
   (list (window-old-buffer other) (eq (window-buffer other) a))))))
'(nil t)
)

(qa
(qa289-fixture (lambda (window a b)
 (let ((other (split-window window 20 'right)))
  (set-window-buffer other a) (set-window-buffer other b) (delete-window other)
  (eq (window-old-buffer other) b))))
'nil
)

(qa
(qa289-fixture (lambda (window _a b)
 (let ((other (split-window window 20 'right)))
  (set-window-buffer other b) (delete-other-windows other) (window-old-buffer other))))
'nil
)

(qa
(qa289-fixture (lambda (window a _b)
 (let* ((other (split-window window 20 'right)) (root (frame-root-window)))
  (set-window-buffer other a)
  (let ((before (window-old-buffer root)))
   (delete-other-windows window) (list before (window-old-buffer root))))))
'(nil nil)
)

(qa
(qa289-fixture (lambda (window a _b)
 (let ((other (split-window window 20 'right)))
  (set-window-buffer other a) (kill-buffer a) (window-old-buffer other))))
'nil
)

(qa
(qa289-fixture (lambda (window a b)
 (let ((other (split-window window 20 'right)))
  (set-window-buffer other a)
  (let ((state (window-state-get other t)))
   (set-window-buffer other b) (window-state-put state other)
   (list (window-old-buffer other) (eq (window-buffer other) a))))))
'(nil t)
)

(qa
(condition-case err (window-old-buffer 17) (error (list (car err) (cadr err))))
'(wrong-type-argument windowp)
)

(qa
(let ((old (frame-window-state-change)))
 (unwind-protect
  (list (set-frame-window-state-change nil t) (frame-window-state-change)
        (set-frame-window-state-change nil nil) (frame-window-state-change))
  (set-frame-window-state-change nil old)))
'(t t nil nil)
)

(qa
(qa289-fixture (lambda (window _a b)
 (let ((other (split-window window 20 'right)))
  (set-window-buffer other b) (delete-window other)
  (let ((before (window-old-buffer other)))
   (kill-buffer b) (list before (window-old-buffer other))))))
'(t t)
)

(princ (qa--done))
