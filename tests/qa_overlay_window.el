;;; qa_overlay_window.el --- Window-specific effective properties -*- lexical-binding: t; -*-
;; Selected/other windows filter overlays; buffer/nil/omitted objects do not.

(defun qa313-window (which &optional category shadow lower text away)
  (let (away-buffer)
    (unwind-protect
        (save-window-excursion
          (with-temp-buffer
            (insert "abc") (switch-to-buffer (current-buffer))
            (let* ((selected (selected-window)) (other (split-window))
                   (o (make-overlay 1 4)) (low (and lower (make-overlay 1 4)))
                   (restriction
                    (cond ((eq which 'selected) selected)
                          ((eq which 'other) other)
                          ((eq which 'invalid) t)
                          ((eq which 'dead)
                           (let ((dead other))
                             (delete-window other) (setq other (split-window)) dead))
                          (t nil))))
              (when away
                (setq away-buffer (generate-new-buffer " *qa313-away*"))
                (with-current-buffer away-buffer (insert "XYZ"))
                (set-window-buffer other away-buffer))
              (overlay-put o 'face 'bold) (overlay-put o 'priority 10)
              (if category
                  (let ((cat (make-symbol "qa313-category")))
                    (put cat 'window restriction) (overlay-put o 'category cat))
                (overlay-put o 'window restriction))
              (when shadow (overlay-put o 'window nil))
              (when low (overlay-put low 'face 'italic) (overlay-put low 'priority 1))
              (when text (put-text-property 1 4 'face 'underline))
              (mapcar
               (lambda (object)
                 (let* ((omitted (eq object 'omitted))
                        (value (if omitted (get-char-property 2 'face)
                                 (get-char-property 2 'face object)))
                        (position-value (if omitted (get-pos-property 2 'face)
                                          (get-pos-property 2 'face object)))
                        (pair (if omitted (get-char-property-and-overlay 2 'face)
                                (get-char-property-and-overlay 2 'face object)))
                        (owner (cdr pair)))
                   (list value position-value (car pair)
                         (cond ((eq owner o) 'a)
                               ((and low (eq owner low)) 'b) (t nil)))))
               (list selected other (current-buffer) nil 'omitted)))))
      (when (and away-buffer (buffer-live-p away-buffer)) (kill-buffer away-buffer)))))

;; other window
(qa (qa313-window 'other)
    '((nil bold nil nil) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; selected window
(qa (qa313-window 'selected)
    '((bold bold bold a) (nil bold nil nil) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; all windows
(qa (qa313-window 'all)
    '((bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; non-window restriction
(qa (qa313-window 'invalid)
    '((bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; deleted window restriction
(qa (qa313-window 'dead)
    '((nil bold nil nil) (nil bold nil nil) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; inherited window restriction
(qa (qa313-window 'other t)
    '((nil bold nil nil) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; nil shadows inherited restriction
(qa (qa313-window 'other t t)
    '((bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; excluded overlay falls to lower
(qa (qa313-window 'other nil nil t)
    '((italic bold italic b) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; excluded overlay falls to text
(qa (qa313-window 'other nil nil nil t)
    '((underline bold underline nil) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; window displays another buffer
(qa (qa313-window 'other nil nil nil nil t)
    '((nil bold nil nil) (nil nil nil nil) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

;; inherited restriction and text fallback
(qa (qa313-window 'other t nil nil t)
    '((underline bold underline nil) (bold bold bold a) (bold bold bold a) (bold bold bold a) (bold bold bold a)))

(princ (qa--done))
