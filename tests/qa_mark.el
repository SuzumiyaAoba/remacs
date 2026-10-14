;; Mark / region domain — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - mark-active is a BUFFER-LOCAL VAR — (mark-active) void-function.
;;   - push-mark returns nil + prints "Mark set".
;;   - set-mark activates the mark (mark-active → t).
;;   - pop-mark restores point to the ring top.
;;   - region-active-p / use-region-p need transient-mark-mode + mark-active.
;;   - region-beginning/end report the active region bounds.
;;   - deactivate-mark/activate-mark toggle mark-active and run their hooks.
;;   - mark-ring-max default = 16; mark-ring collects pushed marks.
;;   - the-region-bounds is NOT a fn in 31.1.
;;   - handle-shift-selection is NOT a var in batch.
;;   - selection-coding-system is NOT a fn in batch.
;;   - transient-mark-mode-line-help-echo is NOT a var.
;;   - gui/x selection fns exist; interprogram-*-function are vars.

;; == mark-active is a var, not a fn ==
(qa-err (mark-active) 'void-function)
(qa (boundp 'mark-active) t)

;; == push-mark / set-mark / mark ==
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (qa (push-mark) nil)
  (qa (mark) 2)
  (qa (markerp (mark-marker)) t))
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (push-mark)
  (qa (marker-position (mark-marker)) 2)
  (qa (marker-buffer (mark-marker)) (current-buffer)))
;; set-mark: position + mark-active on
(with-temp-buffer
  (insert "abc")
  (set-mark 1)
  (qa (mark) 1)
  (qa mark-active t))
;; pop-mark restores point to the ring top
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (push-mark)
  (goto-char 3)
  (push-mark)
  (pop-mark)
  (qa (point) 3))

;; == region-active-p / use-region-p ==
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (set-mark 1)
  (qa (region-active-p) nil)
  (qa (use-region-p) nil))
(with-temp-buffer
  (transient-mark-mode 1)
  (insert "abc")
  (goto-char 2)
  (set-mark 1)
  (setq mark-active t)
  (qa (region-active-p) t)
  (qa (use-region-p) t))
;; region-beginning/end under active mark
(with-temp-buffer
  (transient-mark-mode 1)
  (insert "abc")
  (goto-char 3)
  (set-mark 1)
  (setq mark-active t)
  (qa (region-beginning) 1)
  (qa (region-end) 3))

;; == deactivate-mark / activate-mark toggle mark-active + run hooks ==
(with-temp-buffer
  (transient-mark-mode 1)
  (set-mark 1)
  (setq mark-active t)
  (deactivate-mark)
  (qa mark-active nil)
  (activate-mark)
  (qa mark-active t))
;; activate-mark reactivates even after explicit setq nil
(with-temp-buffer
  (transient-mark-mode 1)
  (set-mark 1)
  (setq mark-active nil)
  (activate-mark)
  (qa mark-active t))
;; hooks fire
(let ((hit nil))
  (with-temp-buffer
    (transient-mark-mode 1)
    (add-hook 'deactivate-mark-hook (lambda () (setq hit t)) nil t)
    (set-mark 1)
    (setq mark-active t)
    (deactivate-mark)
    (qa hit t)))
(let ((hit nil))
  (with-temp-buffer
    (transient-mark-mode 1)
    (add-hook 'activate-mark-hook (lambda () (setq hit t)) nil t)
    (set-mark 1)
    (setq mark-active nil)
    (activate-mark)
    (qa hit t)))

;; == mark-ring ==
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (push-mark)
  (goto-char 3)
  (push-mark)
  (qa (length mark-ring) 1)
  (qa mark-ring-max 16))

;; == mark-ring cons sharing: pop-mark rotates IN PLACE ==
;; GNU's pop-mark is (nconc mark-ring (list (copy-marker (mark-marker))))
;; then (pop mark-ring): a reference captured before the call observes
;; the appended copy, and the popped head marker is detached in place.
(with-temp-buffer
  (insert "abcdefghij")
  (set-mark 5)
  (push-mark 2 t)
  (push-mark 8 t)
  (setq mark-ring nil)
  (push-mark 3 t)
  (let ((captured mark-ring))
    (qa (length captured) 1)
    (qa (marker-position (car captured)) 8)
    (pop-mark)
    ;; The same cons now shows the old head detached + the appended
    ;; copy of the new mark behind it.
    (qa (length captured) 2)
    (qa (marker-buffer (car captured)) nil)
    (qa (marker-position (cadr captured)) 3)
    (qa (length mark-ring) 1)
    (qa (marker-position (car mark-ring)) 3)))

;; == mark-ring-max truncation: in-place cut, marker stays live ==
(with-temp-buffer
  (insert "abcdefghij")
  (let ((mark-ring-max 1)
        (mark-ring nil))
    (set-mark 4)
    (push-mark 2 t)
    ;; The shared cons chain: prepending truncates it in place.
    (let ((captured mark-ring))
      (push-mark 9 t)
      ;; The ring holds the most recent OLD mark (position 2).
      (qa (length mark-ring) 1)
      (qa (marker-position (car mark-ring)) 2)
      ;; GNU's truncation severs the chain at (nthcdr (1- MAX) ring);
      ;; the captured head survives as a one-element list whose marker
      ;; GNU does NOT detach — `old' is the old ring's element at
      ;; index MAX, nil when the ring was short.
      (qa (length captured) 1)
      (qa (marker-position (car captured)) 4)
      (qa (marker-buffer (car captured)) (current-buffer)))))



;; == the-region-bounds doesn't exist ==
(qa (fboundp 'the-region-bounds) nil)

;; == other vars ==
(qa (boundp 'transient-mark-mode) t)
(qa (boundp 'mark-even-if-inactive) t)
(qa (boundp 'deactivate-mark-hook) t)
(qa (boundp 'activate-mark-hook) t)
;; handle-shift-selection is NOT a var under -Q batch (GUI-only)
(qa (boundp 'handle-shift-selection) nil)
(qa (boundp 'select-active-regions) t)
(qa (boundp 'saved-region-selection) t)
(qa (boundp 'transient-mark-mode-line-help-echo) nil)

;; == selection / clipboard fns ==
(qa (fboundp 'gui-get-selection) t)
(qa (fboundp 'gui-set-selection) t)
(qa (fboundp 'x-get-selection) t)
(qa (fboundp 'x-set-selection) t)
(qa (fboundp 'selection-coding-system) nil)
(qa (boundp 'interprogram-cut-function) t)
(qa (boundp 'interprogram-paste-function) t)
(qa (fboundp 'rectangle-mark-mode) t)

;; == push-mark LOCATION NOMSG ACTIVATE — 3rd arg is ACTIVATE ==
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (qa (push-mark nil nil t) nil)          ; ACTIVATE=t
  (qa mark-active t))
;; without ACTIVATE → stays nil
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (qa (push-mark nil t) nil)              ; NOMSG=t only
  (qa mark-active nil))

;; == pop-mark on empty ring → nil (no error) ==
(with-temp-buffer
  (insert "abc")
  (qa (ignore-errors (pop-mark)) nil))

(princ (qa--done))
