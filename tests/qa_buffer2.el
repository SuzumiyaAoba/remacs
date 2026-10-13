;; Buffer machinery (deep) — GNU 31.1 -Q --batch ground truth.
;;
;; GNU batch notes (all verified):
;; - Batch current buffer is *scratch*, EMPTY (size 0), tmm nil,
;;   mark-active nil.  There is one real window showing *scratch*, so
;;   `window-buffer'/`get-buffer-window' DO relate to it; inside
;;   `with-temp-buffer' the temp buffer is NOT in any window.
;; - `set-marker'/`set-mark' CLAMP out-of-range positions into
;;   [point-min, point-max] instead of erroring.
;; - Marker insertion type nil = stays behind text `insert'ed at its
;;   position; `insert-before-markers' advances it.  `copy-marker'
;;   does NOT inherit the source marker's type (always nil unless the
;;   TYPE argument is given).
;; - `set-marker' returns the marker itself; `(set-marker m nil)'
;; - `set-mark' returns nil and ACTIVATES the mark (mark-active t) even
;;   with transient-mark-mode nil; `region-active-p' still nil until
;;   `activate-mark', which sets `transient-mark-mode' to 'lambda.
;;   `deactivate-mark' resets both — but is a no-op when tmm is nil
;;   unless the FORCE argument is given.
;; - `push-mark' pushes the OLD mark onto `mark-ring', then moves the
;;   mark.  `pop-mark' sets mark to the ring head and rotates the ring
;;   (old mark goes to the tail); on an empty ring it is a silent no-op.
;; - `region-beginning' signals plain `error' "The mark is not set now,
;;   so there is no region"; `exchange-point-and-mark' signals
;;   `user-error'.
;; - `buffer-size' ignores narrowing; `narrow-to-region' swaps
;;   reversed BEG/END instead of erroring.
;; - " *temp*" buffers (leading-space name) start with undo DISABLED:
;;   buffer-undo-list is t.  `buffer-disable-undo' returns t;
;;   `buffer-enable-undo' returns nil and resets the list to nil.
;; - `clone-buffer' is NOT an indirect buffer: buffer-base-buffer nil,
;;   text is an independent copy.  `make-indirect-buffer' shares text
;;   but keeps independent point/narrowing.
;; - In a unibyte buffer (insert 255) and (insert "\xff") both add one
;;   byte; in a multibyte buffer char 255 takes two bytes.
;; - `position-bytes'/`byte-to-position' return nil out of range, no error.
;; - `kill-all-local-variables' kills even a `make-variable-buffer-local'
;;   binding; `kill-local-variable' on a never-defvar'd name leaves the
;;   variable globally unbound.

;; Silence "Mark set" message noise from `set-mark'/`push-mark'.
(setq inhibit-message t)

;; == batch buffer state ==
(qa (buffer-name) "*scratch*")
(qa-same (current-buffer) (get-buffer "*scratch*"))
(qa (buffer-size) 0)
(qa (point-min) 1)
(qa (point-max) 1)
(qa (buffer-narrowed-p) nil)
(qa (buffer-live-p (current-buffer)) t)
(qa (buffer-modified-p) nil)
(qa-same (car (buffer-list)) (current-buffer))
(qa (consp (memq (current-buffer) (buffer-list))) t)
(qa (boundp 'initial-scratch-message) t)
(qa (stringp initial-scratch-message) t)
(qa (boundp 'inhibit-startup-screen) t)
(qa inhibit-startup-screen nil)
(qa buffer-read-only nil)
(qa transient-mark-mode nil)
(qa mark-active nil)
(qa mark-ring nil)
(qa mark-ring-max 16)
(qa (fboundp 'scratch-buffer) t)
(qa-same (scratch-buffer) (current-buffer))

;; == batch window relation ==
(qa-same (window-buffer (selected-window)) (current-buffer))
(qa-same (window-buffer) (current-buffer))
(qa-same (get-buffer-window (current-buffer)) (selected-window))
(qa-same (get-buffer-window) (selected-window))
(with-temp-buffer
  ;; temp buffer lives in no window
  (qa (eq (window-buffer (selected-window)) (current-buffer)) nil)
  (qa (get-buffer-window (current-buffer)) nil))
(let ((qa2b-b (generate-new-buffer "qa2b-nowin")))
  (unwind-protect
      (qa (get-buffer-window qa2b-b) nil)
    (kill-buffer qa2b-b)))

;; == marker basics ==
(qa (markerp (make-marker)) t)
(qa (markerp 5) nil)
(qa (markerp nil) nil)
(with-temp-buffer
  (let ((m (make-marker)))
    ;; fresh marker is detached
    (qa (marker-buffer m) nil)
    (qa (marker-position m) nil)
    ;; set-marker returns the marker
    (qa (eq (set-marker m 1) m) t)
    (qa (marker-position m) 1)
    (qa (eq (marker-buffer m) (current-buffer)) t)
    ;; detach: (set-marker m nil)
    (qa (eq (set-marker m nil) m) t)
    (qa (marker-buffer m) nil)
    (qa (marker-position m) nil)))
(with-temp-buffer
  (insert "abc")
  (let ((m (make-marker)))
    ;; positions outside [point-min, point-max] are clamped, no error
    (set-marker m 99)
    (qa (marker-position m) 4)
    (set-marker m 0)
    (qa (marker-position m) 1)))
;; set-marker into a named buffer
(let ((qa2b-b (generate-new-buffer "qa2b-mk")))
  (unwind-protect
      (let ((m (make-marker)))
        (set-marker m 1 qa2b-b)
        (qa (eq (marker-buffer m) qa2b-b) t)
        (qa (marker-position m) 1))
    (kill-buffer qa2b-b)))
;; move-marker is an alias for set-marker
(with-temp-buffer
  (insert "abc")
  (let ((m (make-marker)))
    (qa (eq (move-marker m 4) m) t)
    (qa (marker-position m) 4)))
;; set-marker arg errors
(qa-err-data (set-marker (make-marker) 1 "qa2b-no-such-buf")
             'wrong-type-argument '(bufferp "qa2b-no-such-buf"))
(qa-err-data (copy-marker "notmarker")
             'wrong-type-argument '(integer-or-marker-p "notmarker"))
;; printed form of markers
(qa-prin1 (make-marker) "#<marker in no buffer>")
(with-temp-buffer
  (insert "abc")
  (qa-prin1 (copy-marker 3) "#<marker at 3 in  *temp*>"))

;; == marker insertion semantics ==
;; classic pin: nil-type marker stays BEHIND insert at its position
(with-temp-buffer
  (insert "abcdef")
  (let ((m (copy-marker 3)))
    (qa (marker-insertion-type m) nil)
    (goto-char 3)
    (insert "XY")
    (qa (marker-position m) 3)
    (qa (buffer-string) "abXYcdef")))
;; type t advances with insert at its position
(with-temp-buffer
  (insert "abcdef")
  (let ((m (copy-marker 3 t)))
    (qa (marker-insertion-type m) t)
    (goto-char 3)
    (insert "Q")
    (qa (marker-position m) 4)
    (qa (buffer-string) "abQcdef")))
;; insert-before-markers moves a nil-type marker forward
(with-temp-buffer
  (insert "abcdef")
  (let ((m (copy-marker 3)))
    (goto-char 3)
    (qa (insert-before-markers "Z") nil)
    (qa (marker-position m) 4)
    (qa (buffer-string) "abZcdef")))
;; copy-marker does NOT inherit the source marker's insertion type
(with-temp-buffer
  (insert "abcdef")
  (let ((m (copy-marker 3 t)))
    (qa (marker-insertion-type (copy-marker m)) nil)
    (qa (marker-insertion-type (copy-marker m t)) t)
    (qa (marker-position (copy-marker m)) 3)))
;; set-marker-insertion-type returns the new type
(with-temp-buffer
  (insert "abcdef")
  (let ((m (copy-marker 3)))
    (qa (set-marker-insertion-type m t) t)
    (qa (marker-insertion-type m) t)
    (goto-char 3)
    (insert "Q")
    (qa (marker-position m) 4)))
;; point-marker / point-min-marker / point-max-marker
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (let ((m (point-marker)))
    (qa (markerp m) t)
    (qa (marker-position m) 2)
    (qa (marker-insertion-type m) nil)
    (qa (eq (marker-buffer m) (current-buffer)) t)))
(qa (fboundp 'point-min-marker) t)
(qa (fboundp 'point-max-marker) t)
(with-temp-buffer
  (insert "abc")
  (qa (marker-position (point-min-marker)) 1)
  (qa (marker-position (point-max-marker)) 4)
  ;; fresh marker objects on each call
  (qa (eq (point-min-marker) (point-min-marker)) nil))

;; == mark / mark ring ==
(with-temp-buffer
  ;; no mark: mark returns nil (tmm is nil in batch so no error)
  (qa (mark) nil)
  (qa (mark t) nil)
  ;; mark-marker is a detached marker when the mark is unset
  (qa (marker-position (mark-marker)) nil)
  (qa (marker-buffer (mark-marker)) nil))
(with-temp-buffer
  (insert "abcdefghij")
  ;; set-mark returns nil but sets position AND activates
  (qa (set-mark 3) nil)
  (qa (mark t) 3)
  (qa (mark) 3)
  (qa mark-active t)
  ;; ...but region-active-p is still nil without tmm
  (qa (region-active-p) nil)
  (qa (use-region-p) nil)
  (qa (marker-position (mark-marker)) 3)
  ;; mark-marker is the same internal object
  (qa (eq (mark-marker) (mark-marker)) t)
  (qa (marker-insertion-type (mark-marker)) nil))
;; push-mark pushes the OLD mark onto the ring
(with-temp-buffer
  (insert "abcdefghij")
  (set-mark 1)
  (qa (push-mark 3) nil)
  (qa (mark t) 3)
  (qa (mapcar #'marker-position mark-ring) '(1))
  (push-mark 5)
  (qa (mark t) 5)
  (qa (mapcar #'marker-position mark-ring) '(3 1)))
;; push-mark with no old mark leaves the ring empty
(with-temp-buffer
  (insert "abcdefghij")
  (push-mark 4)
  (qa (mark t) 4)
  (qa mark-ring nil))
;; push-mark defaults to point; third arg ACTIVATE
(with-temp-buffer
  (insert "abcdefghij")
  (goto-char 4)
  (set-mark 2)
  (push-mark)
  (qa (mark t) 4)
  (qa (mapcar #'marker-position mark-ring) '(2))
  (push-mark 6 nil t)
  (qa mark-active t))
;; pop-mark: mark := ring head, ring rotates (old mark to tail);
;; silent no-op on an empty ring
(let ((qa2b-b (generate-new-buffer "qa2b-pop")))
  (unwind-protect
      (with-current-buffer qa2b-b
        (insert "abcdefghij")
        (set-mark 1)
        (push-mark 3)
        (push-mark 5)
        (push-mark 7)
        (qa (mapcar #'marker-position mark-ring) '(5 3 1))
        (qa (marker-position (mark-marker)) 7)
        (qa (pop-mark) nil)
        (qa (marker-position (mark-marker)) 5)
        (qa (mapcar #'marker-position mark-ring) '(3 1 7))
        (pop-mark)
        (qa (marker-position (mark-marker)) 3))
    (kill-buffer qa2b-b)))
(with-temp-buffer
  (insert "abcdefghij")
  (qa (pop-mark) nil)                 ; empty ring, no error
  (set-mark 4)
  (pop-mark)
  (qa (mark t) 4))                    ; empty ring keeps current mark
;; activate-mark sets tmm to 'lambda; deactivate-mark clears both —
;; but deactivate-mark is a no-op while tmm is nil (no FORCE arg)
(with-temp-buffer
  (insert "abc")
  (set-mark 2)
  (deactivate-mark)
  (qa mark-active t)                  ; no-op without tmm
  (qa (deactivate-mark t) nil)        ; FORCE deactivates anyway
  (qa mark-active nil)
  (qa (activate-mark) nil)
  (qa mark-active t)
  (qa transient-mark-mode 'lambda)
  (qa (region-active-p) t)
  (qa (use-region-p) t)
  (qa (deactivate-mark) nil)
  (qa mark-active nil)
  (qa transient-mark-mode nil)
  (qa (region-active-p) nil)
  ;; the mark position survives deactivation
  (qa (mark t) 2))
;; mark-ring is capped at mark-ring-max (16)
(with-temp-buffer
  (insert "abcdefghijklmnopqrst")
  (set-mark 1)
  (dotimes (qa2b-i 20)
    (push-mark (1+ qa2b-i)))
  (qa (length mark-ring) 16))
;; exchange-point-and-mark swaps and ACTIVATES the mark
(with-temp-buffer
  (insert "abcdef")
  (goto-char 2)
  (set-mark 5)
  (setq mark-active nil)
  (qa (exchange-point-and-mark) nil)
  (qa (point) 5)
  (qa (mark t) 2)
  (qa mark-active t)
  ;; swap back
  (exchange-point-and-mark)
  (qa (list (point) (mark t)) '(2 5)))
(with-temp-buffer
  (qa-err (exchange-point-and-mark) 'user-error))
;; region-active-p/use-region-p under an explicit tmm binding
(with-temp-buffer
  (insert "abc")
  (set-mark 2)
  (qa (let ((transient-mark-mode t))
        (list (region-active-p) (use-region-p)))
      '(t t)))

;; == region ==
(with-temp-buffer
  (insert "abcdef")
  (qa-err-data (region-beginning)
               'error '("The mark is not set now, so there is no region"))
  (qa-err (region-end) 'error))
(with-temp-buffer
  (insert "abcdef")
  (goto-char 6)
  (set-mark 2)
  (qa (region-beginning) 2)
  (qa (region-end) 6)
  ;; swaps when point is before the mark
  (goto-char 1)
  (qa (region-beginning) 1)
  (qa (region-end) 2))

;; == narrowing ==
(with-temp-buffer
  (insert "abcdef")
  (qa (narrow-to-region 2 5) nil)
  (qa (point-min) 2)
  (qa (point-max) 5)
  ;; buffer-size ignores narrowing — total char count
  (qa (buffer-size) 6)
  (qa (buffer-narrowed-p) t)
  (qa (buffer-string) "bcd")
  (qa (widen) nil)
  (qa (point-max) 7)
  (qa (buffer-narrowed-p) nil)
  (qa (buffer-string) "abcdef"))
;; reversed bounds are swapped silently
(with-temp-buffer
  (insert "abcdef")
  (narrow-to-region 5 2)
  (qa (list (point-min) (point-max)) '(2 5)))
(with-temp-buffer
  (insert "abcdef")
  (qa-err (narrow-to-region 0 3) 'args-out-of-range)
  (qa-err (narrow-to-region 2 99) 'args-out-of-range)
  (qa-err (narrow-to-region "a" 3) 'wrong-type-argument))

;; == modified flag / ticks ==
(with-temp-buffer
  (qa (buffer-modified-p) nil)
  (insert "x")
  (qa (buffer-modified-p) t)
  (qa (set-buffer-modified-p nil) nil)
  (qa (buffer-modified-p) nil)
  (qa (set-buffer-modified-p t) nil)
  (qa (buffer-modified-p) t)
  (qa (restore-buffer-modified-p nil) nil)
  (qa (buffer-modified-p) nil)
  ;; restore-buffer-modified-p returns its argument
  (qa (restore-buffer-modified-p t) t)
  (qa (buffer-modified-p) t))
;; set-buffer-modified-p t actually flips the flag (was broken:
;; save_tick = mod_tick kept modified-p nil)
(with-temp-buffer
  (qa (set-buffer-modified-p t) nil)
  (qa (buffer-modified-p) t)
  (qa (set-buffer-modified-p nil) nil)
  (qa (buffer-modified-p) nil))
;; indirect buffer shares the modified state with its base
(let* ((b (get-buffer-create "qa2b-mod"))
       (i2 (make-indirect-buffer b "qa2b-mod-i")))
  (unwind-protect
      (progn
        (with-current-buffer i2 (set-buffer-modified-p t))
        (qa (buffer-modified-p b) t)
        (with-current-buffer i2 (set-buffer-modified-p nil))
        (qa (buffer-modified-p b) nil))
    (kill-buffer i2)
    (kill-buffer b)))

;; ticks: both bump on a char insert; set-buffer-modified-p does NOT
(with-temp-buffer
  (let ((t1 (buffer-modified-tick))
        (c1 (buffer-chars-modified-tick)))
    (qa (integerp t1) t)
    (insert "x")
    (qa (< t1 (buffer-modified-tick)) t)
    (qa (< c1 (buffer-chars-modified-tick)) t)
    (let ((t2 (buffer-modified-tick))
          (c2 (buffer-chars-modified-tick)))
      (set-buffer-modified-p nil)
      (qa (= t2 (buffer-modified-tick)) t)
      (qa (= c2 (buffer-chars-modified-tick)) t))))
(qa (>= (buffer-modified-tick) (buffer-chars-modified-tick)) t)

;; == undo enable/disable ==
(with-temp-buffer
  ;; fresh " *temp*" buffers (leading-space name) start with undo
  ;; disabled: buffer-undo-list is t
  (qa buffer-undo-list t)
  (qa (buffer-enable-undo) nil)
  (qa buffer-undo-list nil)
  (insert "x")
  (undo-boundary)
  (qa (consp buffer-undo-list) t)
  (qa (buffer-disable-undo) t)
  (qa buffer-undo-list t)
  (qa (buffer-disable-undo) t))       ; idempotent

;; == buffer-local variables ==
(defvar-local qa2b-lv 10)
(qa (default-value 'qa2b-lv) 10)
(qa (default-boundp 'qa2b-lv) t)
(qa fill-column 70)
(qa (default-value 'fill-column) 70)
(with-temp-buffer
  (qa qa2b-lv 10)
  ;; buffer-local setq does not touch the default
  (setq qa2b-lv 20)
  (qa qa2b-lv 20)
  (qa (default-value 'qa2b-lv) 10)
  (qa (local-variable-p 'qa2b-lv) t)
  (qa (local-variable-p 'qa2b-nosuch) nil)
  ;; automatically-buffer-local var has no local binding until set
  (qa (local-variable-p 'fill-column) nil)
  (qa (local-variable-if-set-p 'fill-column) t)
  (qa (local-variable-if-set-p 'qa2b-lv) t)
  (qa (local-variable-if-set-p 'qa2b-nosuch) nil)
  (qa (eq (variable-binding-locus 'qa2b-lv) (current-buffer)) t)
  (qa (variable-binding-locus 'qa2b-nosuch) nil)
  ;; setq-default returns the value, affects the default binding only
  (qa (setq-default qa2b-lv 77) 77)
  (qa (default-value 'qa2b-lv) 77)
  (qa qa2b-lv 20))
(qa (default-value 'qa2b-lv) 77)
;; make-local-variable returns the symbol; kill-local-variable too
(with-temp-buffer
  (qa (make-local-variable 'qa2b-ml) 'qa2b-ml)
  (setq qa2b-ml 5)
  (qa qa2b-ml 5)
  (qa (local-variable-p 'qa2b-ml) t)
  (qa (kill-local-variable 'qa2b-ml) 'qa2b-ml)
  (qa (local-variable-p 'qa2b-ml) nil)
  ;; var was never defvar'd: killing the local binding leaves it unbound
  (qa (boundp 'qa2b-ml) nil))
(qa (boundp 'qa2b-ml) nil)
;; make-variable-buffer-local: if-set without local binding
(with-temp-buffer
  (qa (make-variable-buffer-local 'qa2b-mvbl) 'qa2b-mvbl)
  (qa (local-variable-p 'qa2b-mvbl) nil)
  (qa (local-variable-if-set-p 'qa2b-mvbl) t))
;; kill-all-local-variables clears local bindings
(with-temp-buffer
  (setq qa2b-al 1)
  (qa (local-variable-p 'qa2b-al) nil) ; plain setq doesn't make it local
  (make-local-variable 'qa2b-al2)
  (setq qa2b-al2 2)
  (kill-local-variable 'qa2b-al2)
  (make-local-variable 'qa2b-al3)
  (setq qa2b-al3 3)
  (qa (kill-all-local-variables) nil)
  (qa (local-variable-p 'qa2b-al3) nil))
;; kill-all-local-variables kills even make-variable-buffer-local bindings
(with-temp-buffer
  (make-variable-buffer-local 'qa2b-perm)
  (setq qa2b-perm 9)
  (kill-all-local-variables)
  (qa (local-variable-p 'qa2b-perm) nil))
;; buffer-local-value / buffer-local-variables across buffers
(let ((qa2b-b (generate-new-buffer "qa2b-lv")))
  (unwind-protect
      (progn
        (qa (buffer-local-value 'fill-column qa2b-b) 70)
        (with-current-buffer qa2b-b (setq fill-column 42))
        (qa (buffer-local-value 'fill-column qa2b-b) 42)
        (qa fill-column 70)
        (qa (assq 'fill-column (buffer-local-variables qa2b-b))
            '(fill-column . 42))
        ;; buffer-file-name is locally bound (but nil) in any buffer
        (qa (assq 'buffer-file-name (buffer-local-variables qa2b-b))
            '(buffer-file-name)))
    (kill-buffer qa2b-b)))

;; == buffer names / creation ==
(qa (get-buffer "qa2b-no-such") nil)
(qa (generate-new-buffer-name "qa2b-zzz-noexist") "qa2b-zzz-noexist")
(let ((qa2b-b (generate-new-buffer "qa2b-tmp")))
  (unwind-protect
      (progn
        (qa (buffer-name qa2b-b) "qa2b-tmp")
        (qa (buffer-live-p qa2b-b) t)
        (qa (generate-new-buffer-name "qa2b-tmp") "qa2b-tmp<2>")
        ;; get-buffer-create returns the SAME buffer object
        (qa-same (get-buffer-create "qa2b-tmp") qa2b-b)
        ;; generate-new-buffer uniquifies with <2>
        (let ((qa2b-b2 (generate-new-buffer "qa2b-tmp")))
          (unwind-protect
              (qa (buffer-name qa2b-b2) "qa2b-tmp<2>")
            (kill-buffer qa2b-b2)))
        ;; generate-new-buffer-name appends even to a <N>-suffixed name
        (let ((qa2b-b3 (generate-new-buffer "qa2b-tmp<3>")))
          (unwind-protect
              (qa (generate-new-buffer-name "qa2b-tmp<3>") "qa2b-tmp<3><2>")
            (kill-buffer qa2b-b3))))
    (kill-buffer qa2b-b)))
;; get-buffer / get-buffer-create accept a buffer object unchanged
(let ((qa2b-b (get-buffer-create "qa2b-gc")))
  (unwind-protect
      (progn
        (qa-same (get-buffer qa2b-b) qa2b-b)
        (qa-same (get-buffer-create qa2b-b) qa2b-b)
        (qa (buffer-name qa2b-b) "qa2b-gc"))
    (kill-buffer qa2b-b)))
;; buffer-name requires a buffer argument
(qa-err-data (buffer-name "qa2b-nm")
             'wrong-type-argument '(bufferp "qa2b-nm"))
;; rename-buffer returns the new name; rename to own name is fine
(with-temp-buffer
  (qa (rename-buffer "qa2b-rn") "qa2b-rn")
  (qa (buffer-name) "qa2b-rn")
  (qa (rename-buffer "qa2b-rn") "qa2b-rn"))
(let ((qa2b-b1 (generate-new-buffer "qa2b-rn1"))
      (qa2b-b2 (generate-new-buffer "qa2b-rn2")))
  (unwind-protect
      (with-current-buffer qa2b-b1
        ;; clash without UNIQUE → error; UNIQUE t → suffix
        (qa-err-data (rename-buffer "qa2b-rn2")
                    'error '("Buffer name ‘qa2b-rn2’ is in use"))
        (qa (rename-buffer "qa2b-rn2" t) "qa2b-rn2<2>")
        (qa (buffer-name) "qa2b-rn2<2>"))
    (kill-buffer qa2b-b1)
    (kill-buffer qa2b-b2)))
(qa-err (rename-buffer 42) 'wrong-type-argument)
(qa-err-data (generate-new-buffer-name 42)
             'wrong-type-argument '(stringp 42))

;; == indirect buffers ==
(let ((qa2b-base (generate-new-buffer "qa2b-base")))
  (unwind-protect
      (with-current-buffer qa2b-base
        (insert "hello")
        (let ((qa2b-ind (make-indirect-buffer qa2b-base "qa2b-ind")))
          (unwind-protect
              (progn
                (qa (bufferp qa2b-ind) t)
                (qa (buffer-name qa2b-ind) "qa2b-ind")
                (qa-same (buffer-base-buffer qa2b-ind) qa2b-base)
                ;; base buffer has no base itself
                (qa (buffer-base-buffer qa2b-base) nil)
                ;; shared text, both directions
                (qa (with-current-buffer qa2b-ind (buffer-string)) "hello")
                (insert "!")
                (qa (with-current-buffer qa2b-ind (buffer-string)) "hello!")
                (with-current-buffer qa2b-ind (insert "?"))
                (qa (buffer-string) "hello?!")
                ;; independent point
                (goto-char 3)
                (with-current-buffer qa2b-ind (goto-char 5))
                (qa (list (point) (with-current-buffer qa2b-ind (point)))
                    '(3 5))
                ;; independent narrowing
                (with-current-buffer qa2b-ind (narrow-to-region 1 4))
                (qa (with-current-buffer qa2b-ind (buffer-narrowed-p)) t)
                (qa (buffer-narrowed-p) nil)
                (with-current-buffer qa2b-ind (widen)))
            (kill-buffer qa2b-ind)))
        ;; killing the indirect buffer leaves the base text
        (qa (buffer-live-p qa2b-base) t)
        (qa (buffer-string) "hello?!"))
    (kill-buffer qa2b-base)))
;; indirect-buffer name clash / non-buffer base → error
(with-temp-buffer
  (qa-err (make-indirect-buffer (current-buffer) (buffer-name)) 'error)
  (qa-err (make-indirect-buffer "qa2b-x" "qa2b-y") 'error))
;; clone-buffer exists but makes an INDEPENDENT copy, not indirect
(qa (fboundp 'clone-buffer) t)
(with-temp-buffer
  (insert "zz")
  (let ((qa2b-c (clone-buffer "qa2b-clone")))
    (unwind-protect
        (progn
          (qa (buffer-name qa2b-c) "qa2b-clone")
          (qa (buffer-base-buffer qa2b-c) nil)
          (qa (with-current-buffer qa2b-c (buffer-string)) "zz")
          (with-current-buffer qa2b-c (insert "!"))
          (qa (buffer-string) "zz")
          (qa (with-current-buffer qa2b-c (buffer-string)) "zz!"))
      (kill-buffer qa2b-c))))

;; == buffer-swap-text ==
(let ((qa2b-a (generate-new-buffer "qa2b-sa"))
      (qa2b-b (generate-new-buffer "qa2b-sb")))
  (unwind-protect
      (progn
        (with-current-buffer qa2b-a (insert "AAA"))
        (with-current-buffer qa2b-b (insert "BB"))
        (with-current-buffer qa2b-a
          (qa (buffer-swap-text qa2b-b) nil)
          (qa (buffer-string) "BB"))
        (qa (with-current-buffer qa2b-b (buffer-string)) "AAA"))
    (kill-buffer qa2b-a)
    (kill-buffer qa2b-b)))
;; swapping with self is a no-op; dead buffer errors
(with-temp-buffer
  (insert "cc")
  (qa (buffer-swap-text (current-buffer)) nil)
  (qa (buffer-string) "cc"))
(let ((qa2b-dead (generate-new-buffer "qa2b-dead")))
  (kill-buffer qa2b-dead)
  (qa-err (with-temp-buffer (buffer-swap-text qa2b-dead)) 'error))

;; == buffer list / live / kill / bury ==
(let ((qa2b-b (generate-new-buffer "qa2b-live")))
  (qa (buffer-live-p qa2b-b) t)
  (qa (consp (memq qa2b-b (buffer-list))) t)
  (qa (kill-buffer qa2b-b) t)
  (qa (buffer-live-p qa2b-b) nil)
  (qa (buffer-name qa2b-b) nil)
  (qa (get-buffer "qa2b-live") nil)
  ;; killing an already-dead buffer returns nil, no error
  (qa (kill-buffer qa2b-b) nil)
  (qa (consp (memq qa2b-b (buffer-list))) nil))
(qa (buffer-live-p "qa2b-no-such") nil)
(qa (buffer-live-p nil) nil)
(qa-err-data (kill-buffer "qa2b-no-such-buf")
             'error '("No buffer named qa2b-no-such-buf"))
(let ((qa2b-n0 (length (buffer-list))))
  (let ((qa2b-b (generate-new-buffer "qa2b-cnt")))
    (qa (= (length (buffer-list)) (1+ qa2b-n0)) t)
    (kill-buffer qa2b-b)))
(qa (bufferp (car (buffer-list))) t)
;; bury-buffer returns nil and moves the buffer to the list tail
(let ((qa2b-b (generate-new-buffer "qa2b-bury")))
  (unwind-protect
      (progn
        (qa (bury-buffer qa2b-b) nil)
        (qa-same (car (last (buffer-list))) qa2b-b))
    (kill-buffer qa2b-b)))
;; other-buffer is never the current buffer; unbury-buffer is live
(qa (bufferp (other-buffer)) t)
(qa (eq (other-buffer) (current-buffer)) nil)
(qa (bufferp (unbury-buffer)) t)

;; == set-buffer / with-current-buffer / save-excursion ==
(let ((qa2b-b (get-buffer-create "qa2b-sb")))
  (unwind-protect
      (progn
        ;; set-buffer returns the buffer; accepts object or name
        (qa-same (set-buffer qa2b-b) qa2b-b)
        (qa (eq (current-buffer) qa2b-b) t)
        (qa-same (set-buffer "qa2b-sb") qa2b-b)
        (set-buffer "*scratch*"))
    (kill-buffer qa2b-b)))
(qa-same (current-buffer) (get-buffer "*scratch*"))
(qa-err-data (set-buffer "qa2b-no-buffer")
             'error '("No buffer named qa2b-no-buffer"))
(qa-err-data (set-buffer 42) 'wrong-type-argument '(stringp 42))
;; with-current-buffer yields last body value and restores
(qa (with-current-buffer "*scratch*" (buffer-name)) "*scratch*")
(let ((qa2b-b (generate-new-buffer "qa2b-wc")))
  (unwind-protect
      (qa (list (with-current-buffer qa2b-b (buffer-name))
                (buffer-name))
          '("qa2b-wc" "*scratch*"))
    (kill-buffer qa2b-b)))
;; save-current-buffer: value of last form, current buffer restored
(let ((qa2b-here (current-buffer)))
  (qa (save-current-buffer (set-buffer "*scratch*") 7) 7)
  (qa (eq (current-buffer) qa2b-here) t))
;; with-temp-buffer: hidden " *temp*" name, unmodified, body value
(qa (with-temp-buffer (buffer-name)) " *temp*")
(qa (with-temp-buffer (insert "ab") (buffer-string)) "ab")
(qa (with-temp-buffer (buffer-modified-p)) nil)
(qa (with-temp-buffer 42) 42)
;; save-excursion restores point but returns last body value
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (qa (save-excursion (goto-char 3) (point)) 3)
  (qa (point) 2)
  (qa (save-excursion (end-of-line) 99) 99))
(qa (goto-char 1) 1)

;; == buffer text access ==
(with-temp-buffer
  (insert "hello")
  (qa (buffer-string) "hello")
  (qa (buffer-substring 1 3) "he")
  (qa (buffer-substring-no-properties 1 3) "he")
  (qa (equal (buffer-string)
             (buffer-substring (point-min) (point-max)))
      t)
  (qa-err (buffer-substring 0 3) 'args-out-of-range)
  (qa-err (buffer-substring 2 9) 'args-out-of-range)
  (qa-err (buffer-substring "x" 3) 'wrong-type-argument))
(with-temp-buffer
  (insert "a")
  (qa (erase-buffer) nil)
  (qa (buffer-size) 0))

;; == multibyte ==
(qa enable-multibyte-characters t)
(with-temp-buffer
  (qa enable-multibyte-characters t)
  ;; set-buffer-multibyte returns the flag value
  (qa (set-buffer-multibyte t) t)
  (qa (set-buffer-multibyte nil) nil)
  (qa enable-multibyte-characters nil))
(with-temp-buffer
  ;; multibyte: é takes 2 bytes → byte count exceeds char count
  (insert "héllo")
  (qa (buffer-size) 5)
  (qa (position-bytes (point-max)) 7)
  (qa (position-bytes 2) 2)
  (qa (byte-to-position 3) 2)
  (qa (position-bytes 1) 1)
  (qa (byte-to-position 1) 1))
(with-temp-buffer
  (set-buffer-multibyte nil)
  ;; unibyte: char 255 and "\xff" are each one raw byte
  (insert 255)
  (qa (buffer-size) 1)
  (qa (position-bytes 2) 2)
  (insert "\xff")
  (qa (buffer-size) 2)
  (qa (position-bytes 3) 3)
  (qa (string-to-list (buffer-string)) '(255 255)))
(with-temp-buffer
  ;; multibyte: char 255 encodes as two bytes
  (insert "\xff")
  (qa (buffer-size) 1)
  (qa (position-bytes 2) 3))
(with-temp-buffer
  (insert (unibyte-string 255))
  (qa (buffer-size) 1)
  (qa (position-bytes 2) 3))
;; out-of-range positions return nil rather than erroring
(with-temp-buffer
  (insert "ab")
  (qa (position-bytes 0) nil)
  (qa (position-bytes 99) nil)
  (qa (byte-to-position 0) nil)
  (qa (byte-to-position 3) 3))

(princ (qa--done))
