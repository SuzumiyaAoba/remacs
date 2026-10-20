;;; -*- lexical-binding: t; -*-
;;; Window-state minimum bounds, verified with GNU Emacs 31.1 -Q.
;;; Character and pixel minima differ for minibuffers even in a 1x1 frame.
(defun qa115-minima (&optional window writable)
  (mapcar #'cdr (car (window-state-get window writable))))

(qa (mapcar #'car (car (window-state-get (minibuffer-window) t)))
    '(min-height min-width min-height-ignore min-width-ignore
      min-height-safe min-width-safe min-pixel-height min-pixel-width
      min-pixel-height-ignore min-pixel-width-ignore
      min-pixel-height-safe min-pixel-width-safe))
(qa (qa115-minima (minibuffer-window) t)
    '(4 10 1 2 1 2 1 10 1 2 1 2))
(qa (qa115-minima (minibuffer-window))
    '(4 10 1 2 1 2 1 10 1 2 1 2))
(qa (let ((window-min-height 7) (window-min-width 13))
      (qa115-minima (minibuffer-window) t))
    '(7 13 1 2 1 2 1 13 1 2 1 2))
(qa (let ((window-min-height 1) (window-min-width 1))
      (qa115-minima (minibuffer-window) t))
    '(1 2 1 2 1 2 1 2 1 2 1 2))
(qa (let ((window-safe-min-height 3) (window-safe-min-width 5))
      (qa115-minima (minibuffer-window) t))
    '(4 10 3 5 3 5 3 10 3 5 3 5))
(qa (let ((window-min-height 0) (window-min-width 0)
          (window-safe-min-height 2) (window-safe-min-width 4))
      (qa115-minima (minibuffer-window) t))
    '(2 4 2 4 2 4 2 4 2 4 2 4))
(qa (with-current-buffer (window-buffer (minibuffer-window))
      (let ((mode-line-format '("mode")) (header-line-format '("header"))
            (tab-line-format '("tab")))
        (qa115-minima (minibuffer-window) t)))
    '(4 10 1 2 1 2 1 10 1 2 1 2))
(qa (list (window-min-size (minibuffer-window))
          (window-min-size (minibuffer-window) nil nil t)
          (window-min-size (minibuffer-window) nil t t))
    '(4 1 1))

;; Live user preferences, safe bounds, and buffer decorations.
(qa (default-value 'mode-line-format)
    '("%e" mode-line-front-space mode-line-mule-info mode-line-client
      mode-line-modified mode-line-remote mode-line-window-dedicated
      mode-line-frame-identification mode-line-buffer-identification "   "
      mode-line-position (project-mode-line project-mode-line-format)
      (vc-mode vc-mode) "  " mode-line-modes mode-line-misc-info mode-line-end-spaces))
(qa (qa115-minima nil t) '(4 10 2 2 1 2 4 10 2 2 1 2))
(qa (let ((window-min-height 7) (window-min-width 13))
      (qa115-minima nil t)) '(7 13 2 2 1 2 7 13 2 2 1 2))
(qa (let ((window-safe-min-height 3) (window-safe-min-width 5))
      (qa115-minima nil t)) '(4 10 4 5 3 5 4 10 4 5 3 5))
(qa (let ((mode-line-format nil)) (qa115-minima nil t))
    '(4 10 1 2 1 2 4 10 1 2 1 2))
(qa (let ((mode-line-format '("mode")) (header-line-format '("header"))
          (tab-line-format '("tab")) (window-min-height 1))
      (qa115-minima nil t)) '(4 10 4 2 1 2 4 10 4 2 1 2))
(qa (let ((window-safe-min-height 3) (window-safe-min-width 5))
      (list (window-safe-min-size) (window-safe-min-size nil t)
            (window-safe-min-size nil nil t) (window-safe-min-size nil t t)))
    '(3 5 3 5))

;; Margins affect the ignored minimum but never the safe minimum.
(qa (unwind-protect
        (progn (set-window-margins nil 3 4) (qa115-minima nil t))
      (set-window-margins nil nil nil))
    '(4 10 2 9 1 2 4 10 2 9 1 2))
(qa (unwind-protect
        (progn (set-window-margins nil 3 4)
               (set-window-parameter nil 'min-margins '(0 . 0))
               (qa115-minima nil t))
      (set-window-parameter nil 'min-margins nil)
      (set-window-margins nil nil nil))
    '(4 10 2 2 1 2 4 10 2 2 1 2))
(qa (unwind-protect
        (progn (set-window-parameter nil 'min-margins '(3.5 . 5.5))
               (qa115-minima nil t))
      (set-window-parameter nil 'min-margins nil))
    '(4 11 2 11 1 2 4 11 2 11 1 2))
(qa (unwind-protect
        (progn (set-window-margins nil 3 4)
               (set-window-parameter nil 'min-margins '(nil . 9))
               (qa115-minima nil t))
      (set-window-parameter nil 'min-margins nil)
      (set-window-margins nil nil nil))
    '(4 14 2 14 1 2 4 14 2 14 1 2))

(qa (let ((window-size-fixed 'height)) (qa115-minima nil t))
    '(24 10 2 2 1 2 24 10 2 2 1 2))
(qa (let ((window-size-fixed 'width)) (qa115-minima nil t))
    '(4 80 2 2 1 2 4 80 2 2 1 2))
(qa (let ((window-size-fixed 'bogus)) (qa115-minima nil t))
    '(4 10 2 2 1 2 4 10 2 2 1 2))

;; The target buffer supplies local restrictions, not the current buffer.
(qa (save-window-excursion
      (with-temp-buffer
        (setq-local window-min-height 9)
        (setq-local window-min-width 15)
        (setq-local mode-line-format nil)
        (let ((w (selected-window)))
          (set-window-buffer w (current-buffer))
          (with-current-buffer (get-buffer "*scratch*")
            (qa115-minima w t)))))
    '(9 15 1 2 1 2 9 15 1 2 1 2))

;; Internal nodes add minima along their split axis and take the maximum
;; across it.  IGNORE=WINDOW suppresses only that window's preferences.
(qa (save-window-excursion
      (split-window-below)
      (let ((window-min-height 6) (window-min-width 11))
        (qa115-minima (frame-root-window) t)))
    '(12 11 4 2 2 2 12 11 4 2 2 2))
(qa (save-window-excursion
      (let* ((w (selected-window)) (other (split-window-below))
             (window-min-height 6))
        (list (window-min-size w nil w)
              (window-min-size w nil other)
              (window-min-size (frame-root-window) nil w))))
    '(2 6 8))
(qa (save-window-excursion
      (split-window-below)
      (split-window-right)
      (let ((window-min-height 6) (window-min-width 11))
        (qa115-minima (frame-root-window) t)))
    '(12 22 4 4 2 4 12 22 4 4 2 4))

(princ (qa--done))
