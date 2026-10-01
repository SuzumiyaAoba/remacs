;; Markers, point/mark and narrowing (deep) — GNU 31.1 -Q --batch ground
;; truth.  `qa_buffer2.el' holds the marker/mark-ring/narrowing basics;
;; this file pins the corner semantics.
;;
;; GNU batch notes (all verified):
;; - `set-marker' clamps to the REAL buffer bounds [1, size+1] and
;;   IGNORES narrowing; `goto-char' clamps to the ACCESSIBLE region
;;   instead.  `set-mark'/`push-mark' clamp like set-marker.
;; - `set-marker' accepts a marker as POSITION (copies buffer + pos);
;;   a detached marker position DETACHES the target silently.  A dead
;;   buffer object as BUFFER arg also detaches silently (no error), but
;;   a buffer NAME string signals (wrong-type-argument bufferp …).
;; - A marker's buffer survives `set-marker' without a BUFFER arg only
;;   when it is already in this buffer — actually NO: a bare set-marker
;;   always points the marker at the CURRENT buffer.
;; - `copy-marker' clamps out-of-range positions; copying a marker in a
;;   killed buffer yields a detached marker.  It takes exactly ≤2 args.
;; - Killing a buffer detaches every marker: markerp stays t, but
;;   marker-buffer/marker-position become nil.
;; - `equal' on markers compares buffer+position; `eq' is identity.
;; - `goto-char' returns its argument (markers included); a marker in
;;   ANOTHER buffer contributes only its position — the current buffer
;;   is not switched and that buffer's point does not move.  A marker
;;   pointing nowhere signals (error "Marker does not point anywhere").
;; - `narrow-to-region' pulls an outlying point into the region; it can
;;   WIDEN back out to the true bounds but signals args-out-of-range
;;   beyond them.  Narrowing to the full extent leaves buffer-narrowed-p
;;   nil.  Inserts at the restriction edges land INSIDE it.
;; - `save-restriction' restores the restriction (boundaries track the
;;   underlying text, so markers inside keep their position relative to
;;   it) but does NOT restore point — an outlying point is clamped back.
;; - `save-excursion' restores point AND the current buffer but, in
;;   31.1, does NOT restore the mark; `save-mark-and-excursion' does.
;; - `(mark)' never errors in batch: set-but-inactive under
;;   transient-mark-mode still returns the position; unset → nil.
;;   `mark-inactive' is a defined error symbol regardless.
;; - `(set-mark nil)' unsets the mark and returns the mark marker object
;;   itself (eq (mark-marker)).
;; - `region-beginning'/`region-end' clip a mark lying outside the
;;   restriction to point-min/point-max; exchange-point-and-mark
;;   exchanges with the CLIPPED mark position.
;; - `beginning-of-buffer'/`end-of-buffer' set the mark at the old point
;;   (no mark-ring entry when the mark was unset).
;; - `forward-char'/`backward-char' signal (end-of-buffer nil) /
;;   (beginning-of-buffer nil) at the bounds; overshooting N moves point
;;   to the bound AND signals.  `forward-line' returns the number of
;;   lines NOT moved and is a no-op at buffer end.
;; - `preceding-char'/`following-char' return 0 (not nil) at the bounds;
;;   `char-before'/`char-after' return nil and accept marker args.
;; - `buffer-has-markers-at' and `buffer-mark' do NOT exist in 31.1.

(setq inhibit-message t)               ; silence "Mark set"

;; == marker arg / buffer-arg corners ==
(with-temp-buffer
  (insert "abcd")
  (let ((qam-m1 (copy-marker 3))
        (qam-m2 (make-marker)))
    ;; marker as position arg: copies position AND buffer
    (qa (eq (set-marker qam-m2 qam-m1) qam-m2) t)
    (qa (marker-position qam-m2) 3)
    (qa (eq (marker-buffer qam-m2) (current-buffer)) t)))
(with-temp-buffer
  (insert "abcd")
  (let ((qam-m1 (make-marker))       ; detached marker as position arg
        (qam-m2 (copy-marker 2)))
    (set-marker qam-m2 qam-m1)
    (qa (marker-buffer qam-m2) nil)
    (qa (marker-position qam-m2) nil)))
;; a bare set-marker repoints a foreign marker at the CURRENT buffer
(let ((qam-a (get-buffer-create "qam-a"))
      (qam-b (get-buffer-create "qam-b")))
  (unwind-protect
      (progn
        (with-current-buffer qam-a (insert "xx"))
        (with-current-buffer qam-b (insert "yyyy"))
        (let ((qam-m (with-current-buffer qam-a (copy-marker 2))))
          (with-current-buffer qam-b (set-marker qam-m 3))
          (qa (eq (marker-buffer qam-m) qam-b) t)
          (qa (marker-position qam-m) 3)))
    (kill-buffer qam-a)
    (kill-buffer qam-b)))
;; dead buffer object detaches silently; name string errors
(let ((qam-dead (get-buffer-create "qam-dead")))
  (kill-buffer qam-dead)
  (let ((qam-m (make-marker)))
    (qa (eq (set-marker qam-m 1 qam-dead) qam-m) t)
    (qa (marker-buffer qam-m) nil)
    (qa (marker-position qam-m) nil)))
(let ((qam-live (get-buffer-create "qam-name")))
  (unwind-protect
      (qa-err-data (set-marker (make-marker) 1 "qam-name")
                   'wrong-type-argument '(bufferp "qam-name"))
    (kill-buffer qam-live)))
;; killing a buffer detaches its markers
(let ((qam-b (get-buffer-create "qam-kill")))
  (let ((qam-m (with-current-buffer qam-b (insert "qq") (copy-marker 2))))
    (kill-buffer qam-b)
    (qa (markerp qam-m) t)
    (qa (marker-buffer qam-m) nil)
    (qa (marker-position qam-m) nil)
    ;; copy-marker of a dead marker → detached copy, no error
    (let ((qam-c (copy-marker qam-m)))
      (qa (markerp qam-c) t)
      (qa (marker-buffer qam-c) nil)
      (qa (marker-position qam-c) nil))))
;; copy-marker arity and clamping
(with-temp-buffer
  (insert "ab")
  (qa-err (copy-marker 2 nil (current-buffer)) 'wrong-number-of-arguments)
  (qa (marker-position (copy-marker 99)) 3)
  (qa (marker-position (copy-marker -1)) 1)
  (qa (marker-position (copy-marker 0)) 1))
;; set-marker clamps to REAL bounds, position 0 → 1, point-max+1 → max
(with-temp-buffer
  (insert "ab")
  (let ((qam-m (make-marker)))
    (set-marker qam-m 0)
    (qa (marker-position qam-m) 1)
    (set-marker qam-m 4)
    (qa (marker-position qam-m) 3)
    (qa-err-data (set-marker qam-m 1.5)
                 'wrong-type-argument '(integer-or-marker-p 1.5))))
;; insertion type survives set-marker repositioning
(with-temp-buffer
  (insert "abcd")
  (let ((qam-m (copy-marker 3 t)))
    (set-marker qam-m 2)
    (qa (marker-insertion-type qam-m) t)))
;; detached marker still carries an insertion type
(let ((qam-m (make-marker)))
  (qa (marker-insertion-type qam-m) nil)
  (qa (set-marker-insertion-type qam-m t) t)
  (qa (marker-insertion-type qam-m) t))
;; markers: equal by buffer+position, never eq-fresh
(with-temp-buffer
  (insert "ab")
  (let ((qam-m (copy-marker 2)))
    (qa (equal qam-m (copy-marker 2)) t)
    (qa (eq qam-m (copy-marker 2)) nil)
    (qa (equal qam-m qam-m) t))
  (qa (eq (point-marker) (point-marker)) nil)
  (qa (markerp [1]) nil)
  (qa (markerp (make-bool-vector 2 t)) nil))
;; accessor arg errors
(qa-err-data (marker-position 5) 'wrong-type-argument '(markerp 5))
(qa-err-data (marker-buffer "x") 'wrong-type-argument '(markerp "x"))
(qa-err (marker-insertion-type 2) 'wrong-type-argument)
(qa-err (set-marker-insertion-type 3 t) 'wrong-type-argument)

;; == goto-char ==
(with-temp-buffer
  (insert "abc")
  (qa (goto-char 2) 2)
  (qa (point) 2)
  ;; return value is the argument itself, markers included
  (let ((qam-m (copy-marker 1)))
    (qa-same (goto-char qam-m) qam-m))
  (qa (point) 1)
  ;; out-of-range clamps silently, returns the ORIGINAL argument
  (qa (goto-char 99) 99)
  (qa (point) 4)
  (qa (goto-char 0) 0)
  (qa (point) 1)
  (qa-err-data (goto-char "x") 'wrong-type-argument '(integer-or-marker-p "x"))
  (qa-err-data (goto-char nil) 'wrong-type-argument '(integer-or-marker-p nil))
  (qa-err (goto-char 1.5) 'wrong-type-argument))
;; marker pointing nowhere → error
(qa-err-data (with-temp-buffer (goto-char (make-marker)))
             'error '("Marker does not point anywhere"))
;; a marker in another buffer contributes only its position:
;; current buffer is NOT switched, other buffer's point is NOT moved
(let ((qam-b (get-buffer-create "qam-gc")))
  (unwind-protect
      (progn
        (with-current-buffer qam-b (insert "xyzzy") (goto-char 1))
        (let ((qam-m (with-current-buffer qam-b (copy-marker 4))))
          (qa (goto-char qam-m) qam-m)
          (qa (buffer-name) "*scratch*")
          (qa (point) 1)
          (qa (with-current-buffer qam-b (point)) 1)))
    (kill-buffer qam-b)))
;; goto-char on point-min-marker / point-max-marker
(with-temp-buffer
  (insert "ab")
  (qa (markerp (goto-char (point-max-marker))) t)
  (qa (point) 3)
  (qa (markerp (goto-char (point-min-marker))) t)
  (qa (point) 1))
(qa (fboundp 'point) t)

;; == narrowing vs point & markers ==
(with-temp-buffer
  (insert "0123456789")
  ;; point above the new region is pulled DOWN to point-max
  (goto-char 9)
  (narrow-to-region 3 7)
  (qa (point) 7)
  (widen)
  ;; point below the new region is pulled UP to point-min
  (goto-char 2)
  (narrow-to-region 4 7)
  (qa (point) 4)
  (qa (point-min) 4))
;; goto-char clamps to the ACCESSIBLE region…
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (goto-char 1)
  (qa (point) 3)
  (goto-char 20)
  (qa (point) 7))
;; …but set-marker ignores the restriction, clamping to real bounds
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (let ((qam-m (make-marker)))
    (set-marker qam-m 1)
    (qa (marker-position qam-m) 1)
    (set-marker qam-m 20)
    (qa (marker-position qam-m) 11)))
;; a pre-existing marker keeps its absolute position outside the region
(with-temp-buffer
  (insert "0123456789")
  (let ((qam-m (copy-marker 9)))
    (narrow-to-region 3 7)
    (qa (marker-position qam-m) 9)
    (qa (eq (marker-buffer qam-m) (current-buffer)) t)))
;; narrowing to the full extent is NOT "narrowed"
(with-temp-buffer
  (insert "abcd")
  (narrow-to-region 1 5)
  (qa (point-min) 1)
  (qa (point-max) 5)
  (qa (buffer-narrowed-p) nil))
;; re-narrow can widen out to the true bounds…
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (narrow-to-region 1 11)
  (qa (list (point-min) (point-max)) '(1 11)))
;; …but not beyond them (checked against REAL bounds, not the old region)
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (qa-err (narrow-to-region 1 12) 'args-out-of-range)
  (qa-err (narrow-to-region 0 5) 'args-out-of-range))
;; inserts at the restriction edges land INSIDE the region
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (goto-char 3)
  (insert "X")
  (qa (point-min) 3)
  (qa (point-max) 8)
  (qa (buffer-string) "X2345")
  (qa (point) 4))
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (goto-char 7)
  (insert "X")
  (qa (buffer-string) "2345X")
  (qa (point-max) 8))
;; delete-region may not reach outside the restriction
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (qa-err (delete-region 1 4) 'args-out-of-range)
  (qa-err (delete-region 6 10) 'args-out-of-range)
  (qa (buffer-string) "2345"))
;; erase-buffer wipes the WHOLE buffer even while narrowed
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (erase-buffer)
  (qa (point-min) 1)
  (qa (point-max) 1)
  (qa (buffer-size) 0)
  (qa (buffer-narrowed-p) nil))
;; narrow inside the region tracks marker-style boundaries
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (delete-region 3 4)
  (qa (point-min) 3)
  (qa (point-max) 6)
  (qa (buffer-string) "345"))
;; point-min-marker / point-max-marker / point-marker under narrowing
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (qa (marker-position (point-min-marker)) 3)
  (qa (marker-position (point-max-marker)) 7)
  (goto-char 20)
  (qa (marker-position (point-marker)) 7))
;; widening keeps point
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (goto-char 5)
  (widen)
  (qa (point) 5)
  (qa (widen) nil))                 ; idempotent

;; == save-restriction / with-restriction ==
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (goto-char 4)
  ;; body may widen/move; bounds restored, point clamped back inside
  (qa (save-restriction
        (widen)
        (goto-char 9)
        (insert "!"))
      nil)
  (qa (point-min) 3)
  (qa (point-max) 7)
  (qa (point) 7)
  (qa (buffer-string) "2345"))
;; restriction boundaries track edits relative to the text
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (let ((qam-m (copy-marker 4)))
    (save-restriction
      (widen)
      (delete-region 1 3))          ; drop 2 chars before the region
    (qa (point-min) 1)
    (qa (point-max) 5)
    (qa (marker-position qam-m) 2)
    (qa (buffer-string) "2345")))
;; save-restriction restores an OUTER narrower region too
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (qa (save-restriction (narrow-to-region 4 6) 'qam-val) 'qam-val)
  (qa (point-min) 3)
  (qa (point-max) 7))
;; …and restores "not narrowed" state
(with-temp-buffer
  (insert "0123456789")
  (save-restriction (narrow-to-region 3 7))
  (qa (point-min) 1)
  (qa (point-max) 11)
  (qa (buffer-narrowed-p) nil))
;; with-restriction macro: narrows, restores
(with-temp-buffer
  (insert "012345")
  (qa (with-restriction 2 5 (list (point-min) (point-max))) '(2 5))
  (qa (point-min) 1)
  (qa (point-max) 7))
(qa (macrop 'with-restriction) t)

;; == save-excursion / save-mark-and-excursion ==
(with-temp-buffer
  (insert "abcd")
  (goto-char 2)
  ;; 31.1: save-excursion restores point but NOT the mark
  (set-mark 4)
  (qa (save-excursion (goto-char 3) (set-mark 1) 'done) 'done)
  (qa (point) 2)
  (qa (mark t) 1)
  (qa mark-active t))
;; but it DOES restore the current buffer
(let ((qam-b (get-buffer-create "qam-se")))
  (unwind-protect
      (progn
        (qa (with-temp-buffer
              (save-excursion (set-buffer qam-b) 'inner))
            'inner)
        (qa (buffer-name) "*scratch*"))
    (kill-buffer qam-b)))
;; save-mark-and-excursion restores point AND mark
(with-temp-buffer
  (insert "abcd")
  (goto-char 2)
  (set-mark 4)
  (save-mark-and-excursion
    (goto-char 3)
    (set-mark 1))
  (qa (point) 2)
  (qa (mark t) 4))
(qa (macrop 'save-mark-and-excursion) t)
;; nested save-excursion around save-restriction
(with-temp-buffer
  (insert "012345")
  (narrow-to-region 2 5)
  (goto-char 3)
  (save-excursion
    (save-restriction
      (widen)
      (goto-char 1)))
  (qa (point) 3)
  (qa (point-min) 2)
  (qa (point-max) 5))

;; == mark corners ==
;; (set-mark nil) unsets and returns the mark marker itself
(with-temp-buffer
  (insert "ab")
  (set-mark 2)
  (let ((qam-r (set-mark nil)))
    (qa (markerp qam-r) t)
    (qa-same qam-r (mark-marker))
    (qa (mark t) nil)
    (qa mark-active nil)))
;; set-mark accepts marker args (position only) and clamps
(with-temp-buffer
  (insert "ab")
  (let ((qam-m (copy-marker 2)))
    (qa (set-mark qam-m) nil)
    (qa (mark t) 2))
  (set-mark 99)
  (qa (mark t) 3)
  (set-mark 0)
  (qa (mark t) 1)
  (qa-err-data (set-mark 1.5)
               'wrong-type-argument '(integer-or-marker-p 1.5))
  (qa-err (set-mark "a") 'wrong-type-argument))
;; moving mark-marker directly moves the mark (no activation)
(with-temp-buffer
  (insert "abc")
  (set-marker (mark-marker) 2)
  (qa (mark t) 2)
  (qa mark-active nil))
;; a foreign marker's position is used for set-mark in THIS buffer
(let ((qam-b (get-buffer-create "qam-sm")))
  (unwind-protect
      (progn
        (with-current-buffer qam-b (insert "qq"))
        (with-temp-buffer
          (insert "abcdef")
          (set-mark (with-current-buffer qam-b (copy-marker 2)))
          (qa (mark t) 2)))
    (kill-buffer qam-b)))
;; (mark) never signals mark-inactive in batch — not even with
;; transient-mark-mode on and an inactive mark
(with-temp-buffer
  (insert "ab")
  (let ((transient-mark-mode t))
    (set-mark 2)
    (setq mark-active nil)
    (qa (mark) 2)
    (qa (mark t) 2)))
(with-temp-buffer
  (insert "ab")
  (let ((transient-mark-mode t))
    (qa (mark) nil)))                ; unset → nil, not an error
(with-temp-buffer
  (insert "ab")
  (unwind-protect
      (progn
        (transient-mark-mode 1)
        (set-mark 2)
        (deactivate-mark)
        (qa mark-active nil)
        (qa (mark) 2))
    (transient-mark-mode -1)))
(qa (get 'mark-inactive 'error-conditions) '(mark-inactive error))
;; activate-mark on an unset mark: silent no-op
(with-temp-buffer
  (qa (activate-mark) nil)
  (qa mark-active nil))
;; deactivate-mark with nothing set: silent no-op, with or without FORCE
(with-temp-buffer
  (qa (deactivate-mark) nil)
  (qa (deactivate-mark t) nil)
  (qa mark-active nil))
;; push-mark clamps the new mark like set-mark
(with-temp-buffer
  (insert "ab")
  (push-mark 99)
  (qa (mark t) 3))
;; push-mark pushes the old mark even when the position is unchanged
(with-temp-buffer
  (insert "abc")
  (set-mark 1)
  (push-mark 1)
  (qa (mark t) 1)
  (qa (mapcar #'marker-position mark-ring) '(1)))
;; mark-ring holds real markers in this buffer
(with-temp-buffer
  (insert "abc")
  (set-mark 1)
  (push-mark 3)
  (qa (markerp (car mark-ring)) t)
  (qa (marker-position (car mark-ring)) 1)
  (qa (eq (marker-buffer (car mark-ring)) (current-buffer)) t))
;; pop-mark on an empty ring with no mark: nothing happens
(with-temp-buffer
  (insert "abc")
  (qa (pop-mark) nil)
  (qa (mark t) nil))
;; exchange-point-and-mark with point == mark is a no-op
(with-temp-buffer
  (insert "abc")
  (set-mark 2)
  (goto-char 2)
  (setq mark-active nil)
  (qa (exchange-point-and-mark) nil)
  (qa (point) 2)
  (qa (mark t) 2))

;; == region clipping under narrowing ==
;; a mark outside the restriction is clipped to the boundary
(with-temp-buffer
  (insert "0123456789")
  (set-mark 2)
  (narrow-to-region 4 7)
  (qa (mark t) 2)                   ; the marker itself is untouched
  (qa (region-beginning) 4)
  (qa (region-end) 7))
(with-temp-buffer
  (insert "0123456789")
  (set-mark 9)
  (goto-char 1)
  (narrow-to-region 3 7)
  (qa (region-beginning) 3)
  (qa (region-end) 7))
;; exchange-point-and-mark swaps with the CLIPPED mark
(with-temp-buffer
  (insert "0123456789")
  (set-mark 9)
  (goto-char 4)
  (narrow-to-region 2 6)
  (qa (exchange-point-and-mark) nil)
  (qa (point) 6)
  (qa (mark t) 4))
;; region-* tolerate an inactive mark (only unset signals)
(with-temp-buffer
  (insert "abc")
  (set-mark 2)
  (goto-char 3)
  (setq mark-active nil)
  (qa (region-beginning) 2)
  (qa (region-end) 3)
  (qa mark-active nil))
(qa-err-data (with-temp-buffer (insert "abc") (region-end))
             'error '("The mark is not set now, so there is no region"))
;; mark-whole-buffer: point → point-min, mark pushed at point-max
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (qa (mark-whole-buffer) 1)
  (qa (point) 1)
  (qa (mark t) 4)
  (qa mark-active t))

;; == char accessors at the boundaries ==
(with-temp-buffer
  (insert "ab")
  (goto-char (point-min))
  (qa (char-before) nil)
  (qa (char-after) 97)
  (qa (preceding-char) 0)           ; 0, not nil
  (qa (following-char) 97)
  (goto-char (point-max))
  (qa (char-before) 98)
  (qa (char-after) nil)
  (qa (preceding-char) 98)
  (qa (following-char) 0)
  ;; explicit position args: nil out of bounds, marker args accepted
  (qa (char-before 2) 97)
  (qa (char-after 2) 98)
  (qa (char-after 0) nil)
  (qa (char-before 99) nil)
  (qa (char-after 99) nil)
  (qa (char-after -1) nil)
  (let ((qam-m (copy-marker 2)))
    (qa (char-after qam-m) 98)
    (qa (char-before qam-m) 97))
  (qa-err (char-after "x") 'wrong-type-argument))
;; accessors see only the accessible region
(with-temp-buffer
  (insert "012345")
  (narrow-to-region 3 5)
  (qa (char-before 3) nil)
  (qa (char-after 5) nil)
  (qa (char-before 2) nil)
  (qa (char-after 6) nil)
  (qa (char-before 4) 50))

;; == forward/backward char bounds ==
(with-temp-buffer
  (insert "ab")
  (goto-char 1)
  (qa (forward-char) nil)
  (qa (point) 2)
  (qa (forward-char 0) nil)
  (qa (point) 2)
  (qa-err-data (progn (goto-char 3) (forward-char))
               'end-of-buffer nil)
  ;; overshooting N moves point to the bound AND signals
  (goto-char 1)
  (qa-err (forward-char 99) 'end-of-buffer)
  (qa (point) 3)
  (qa-err (backward-char 5) 'beginning-of-buffer)
  (qa (point) 1)
  (qa (backward-char 0) nil)
  ;; negative forward-char walks backwards
  (goto-char 3)
  (qa (forward-char -1) nil)
  (qa (point) 2)
  (qa (backward-char -1) nil)
  (qa (point) 3))
;; delete-char overshoot: signals end-of-buffer, buffer untouched
(with-temp-buffer
  (insert "ab")
  (qa-err-data (delete-char 99) 'end-of-buffer nil)
  (qa (point-max) 3)
  (qa (buffer-string) "ab")
  (goto-char 1)
  (qa-err (delete-backward-char 1) 'beginning-of-buffer))

;; == line bounds ==
(with-temp-buffer
  (insert "ab\ncd\nef")
  (goto-char 5)
  (qa (bolp) nil)
  (qa (eolp) nil)
  (qa (line-beginning-position) 4)
  (qa (line-end-position) 6)
  ;; N counts from the previous line: 0 → previous line's bounds
  (qa (line-beginning-position 2) 7)
  (qa (line-end-position 2) 9)
  (qa (line-beginning-position 0) 1)
  (qa (line-end-position 0) 3)
  (qa (line-beginning-position -1) 1)
  (goto-char 6)
  (qa (eolp) t)
  (goto-char 4)
  (qa (bolp) t)
  ;; old names and new pos-* names agree
  (qa (point-at-bol) 4)
  (goto-char 5)
  (qa (point-at-eol) 6)
  (qa (pos-bol) 4)
  (qa (pos-eol) 6))
(qa (fboundp 'point-at-bol) t)
(qa (fboundp 'point-at-eol) t)
(qa (fboundp 'pos-bol) t)
(qa (fboundp 'pos-eol) t)
;; last line without trailing newline: lep = point-max
(with-temp-buffer
  (insert "ab\ncd")
  (goto-char 4)
  (qa (line-end-position) 6)
  (qa (line-end-position 2) 6)      ; past last line → point-max
  (goto-char 6)
  (qa (eolp) t))
;; bobp/eobp respect narrowing
(with-temp-buffer
  (insert "012345")
  (narrow-to-region 2 5)
  (goto-char 2)
  (qa (bobp) t)
  (goto-char 5)
  (qa (eobp) t)
  (qa (eolp) t))
;; plain bobp/eobp: insert leaves point at point-max
(with-temp-buffer
  (insert "ab")
  (qa (eobp) t)
  (qa (bobp) nil)
  (goto-char 1)
  (qa (bobp) t)
  (qa (eobp) nil))
;; line bounds never reach outside the restriction
(with-temp-buffer
  (insert "ab\ncd\nef")
  (narrow-to-region 5 8)
  (goto-char 6)
  (qa (line-beginning-position) 5)
  (qa (line-end-position) 6)
  ;; end-of-line may not pass the restriction's ZV either
  (widen))
(with-temp-buffer
  (insert "abcdef")
  (narrow-to-region 2 4)
  (goto-char 2)
  (qa (end-of-line) nil)
  (qa (point) 4)
  (qa (eolp) t))

;; == forward-line / bol-eol movement ==
(with-temp-buffer
  (insert "ab\ncd\nef")
  (goto-char 9)                     ; already at end: nothing moved → 1
  (qa (forward-line) 1)
  (qa (point) 9))
(with-temp-buffer
  (insert "ab\ncd")
  ;; forward-line past the end: lands at eob, returns N minus lines moved
  ;; (moving onto the last, unterminated line still counts as a move)
  (goto-char 1)
  (qa (forward-line 9) 7)
  (qa (point) 6)
  (qa (forward-line 3) 3)
  (qa (point) 6))
(with-temp-buffer
  (insert "ab\ncd\nef")
  (goto-char 6)
  ;; negative → beginning-of-line N lines up
  (qa (forward-line -1) 0)
  (qa (point) 1))
;; forward-line respects the restriction
(with-temp-buffer
  (insert "a\nb\nc\nd")
  (narrow-to-region 3 6)
  (goto-char 3)
  (qa (forward-line) 0)
  (qa (point) 5))
;; beginning-of-line / end-of-line N uses N-1 (0 → previous line)
(with-temp-buffer
  (insert "ab\ncd\nef")
  (goto-char 5)
  (qa (beginning-of-line) nil)
  (qa (point) 4)
  (qa (list (progn (beginning-of-line 0) (point))
            (progn (beginning-of-line -1) (point))
            (progn (end-of-line 0) (point))
            (progn (end-of-line -1) (point)))
      '(1 1 1 1)))
(with-temp-buffer
  (insert "ab\ncd")
  (goto-char 1)
  ;; end-of-line N too large: no error, lands at eob
  (qa (end-of-line 9) nil)
  (qa (point) 6))
(with-temp-buffer
  (insert "ab\ncd")
  (goto-char 2)
  (qa (beginning-of-line -1) nil)   ; before first line: stays at bol
  (qa (point) 1))
;; move-*-of-line are the same commands
(qa (fboundp 'move-end-of-line) t)
(qa (fboundp 'move-beginning-of-line) t)
(with-temp-buffer
  (insert "ab\ncd")
  (goto-char 4)
  (qa (move-end-of-line 1) nil)
  (qa (point) 6))

;; == beginning-of-buffer / end-of-buffer ==
(with-temp-buffer
  (insert "0123456789")
  (goto-char 5)
  (qa (beginning-of-buffer) nil)
  (qa (point) 1)
  ;; non-interactive bob leaves the mark at the OLD point…
  (qa (mark t) 5)
  ;; …without pushing the mark-ring when the mark was unset
  (qa mark-ring nil))
(with-temp-buffer
  (insert "abc")
  (goto-char 1)
  (qa (end-of-buffer) nil)
  (qa (point) 4)
  (qa (mark t) 1))
;; bounds respect the restriction
(with-temp-buffer
  (insert "0123456789")
  (narrow-to-region 3 7)
  (goto-char 6)
  (beginning-of-buffer)
  (qa (point) 3)
  (qa (mark t) 6)
  (end-of-buffer)
  (qa (point) 7))
;; numeric arg: fraction of the buffer (pinned values)
(with-temp-buffer
  (insert "a\nb\nc")
  (qa (beginning-of-buffer 1) 0)
  (qa (point) 3)
  (qa (beginning-of-buffer 10) 1)
  (qa (point) 6))
(with-temp-buffer
  (insert "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n")
  (beginning-of-buffer 5)
  (qa (point) 19)
  (end-of-buffer 3)
  (qa (point) 25))

;; == narrow-to-defun / narrow-to-page ==
(with-temp-buffer
  (insert "(defun a ()\n  1)\n\n(defun b ()\n  2)\n")
  (goto-char 28)                    ; inside the second defun
  (qa (narrow-to-defun) nil)
  (qa (point-min) 19)
  (qa (point-max) 36)
  (qa (buffer-string) "(defun b ()\n  2)\n"))
;; before the first defun: narrows to the first one
(with-temp-buffer
  (insert "text\n\n(defun b ()\n  2)\n")
  (goto-char 2)
  (narrow-to-defun)
  (qa (point-min) 1)
  (qa (point-max) 6))
;; narrow-to-page: ^\f delimits pages
(qa (fboundp 'narrow-to-page) t)
(with-temp-buffer
  (insert "ab\n\fcd\n\fef")
  (goto-char 6)
  (qa (narrow-to-page) nil)
  (qa (point-min) 5)
  (qa (point-max) 8))
(with-temp-buffer
  (insert "abc")
  (narrow-to-page)                  ; no formfeed → whole buffer
  (qa (list (point-min) (point-max) (buffer-narrowed-p)) '(1 4 nil)))

;; == count-lines / line-number-at-pos ==
(with-temp-buffer
  (insert "ab\ncd\nef")
  (qa (count-lines 1 7) 2)
  (qa (count-lines (point-min) (point-max)) 3)
  (qa-err-data (count-lines 12 1) 'args-out-of-range '(12 1))
  (qa-err (count-lines 1 99) 'args-out-of-range))
(with-temp-buffer
  (insert "a\nb\nc")
  (qa (line-number-at-pos 1) 1)
  (qa (line-number-at-pos 5) 3)
  (qa (line-number-at-pos) 3))      ; point at eob
;; by default counts from point-min of the (narrowed) buffer;
;; ABSOLUTE counts from the real buffer start
(with-temp-buffer
  (insert "a\nb\nc\nd\n")
  (narrow-to-region 3 8)
  (qa (line-number-at-pos 6) 2)
  (qa (line-number-at-pos 6 t) 3))

;; == absent in 31.1 ==
(qa (fboundp 'buffer-has-markers-at) nil)
(qa (fboundp 'buffer-mark) nil)

(princ (qa--done))
