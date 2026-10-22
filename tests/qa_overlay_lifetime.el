;;; qa_overlay_lifetime.el --- Detached overlay ownership -*- lexical-binding: t; -*-
;; Properties and insertion flags survive source buffer destruction.

(defun qa307-detached (&optional front rear)
  (let ((buffer (generate-new-buffer " *qa307-source*")))
    (unwind-protect
        (progn
          (with-current-buffer buffer (insert "abcd"))
          (let ((overlay (make-overlay 1 3 buffer front rear)))
            (overlay-put overlay 'x 1)
            overlay))
      (when (buffer-live-p buffer) (kill-buffer buffer)))))

(defun qa307-flags (front rear)
  (with-temp-buffer
    (insert "abcd")
    (let ((overlay (qa307-detached front rear)))
      (move-overlay overlay 2 4 (current-buffer))
      (goto-char 2) (insert "X")
      (goto-char (overlay-end overlay)) (insert "Y")
      (list (overlay-start overlay) (overlay-end overlay)
            (overlay-get overlay 'x)))))

(defun qa307-cross-buffer (old-first)
  (let ((old (generate-new-buffer " *qa307-old*"))
        (new (generate-new-buffer " *qa307-new*")))
    (unwind-protect
        (progn
          (with-current-buffer old (insert "abcd"))
          (with-current-buffer new (insert "abcd"))
          (let ((overlay (make-overlay 1 3 old)))
            (overlay-put overlay 'x 1)
            (move-overlay overlay 2 4 new)
            (overlay-put overlay 'y 2)
            (let ((attached
                   (if old-first
                       (progn (kill-buffer old)
                              (and (eq (overlay-buffer overlay) new)
                                   (= (overlay-get overlay 'y) 2)))
                     t)))
              (kill-buffer new)
              (unless old-first (kill-buffer old))
              (list attached (overlay-buffer overlay) (overlay-start overlay)
                    (overlay-get overlay 'x) (overlay-get overlay 'y)))))
      (when (buffer-live-p old) (kill-buffer old))
      (when (buffer-live-p new) (kill-buffer new)))))

;; state after kill
(qa (condition-case error (let ((o (qa307-detached))) (list (overlayp o) (type-of o) (overlay-buffer o) (overlay-start o) (overlay-end o) (overlay-get o 'x))) (error error))
    '(t overlay nil nil nil 1))

;; put after kill
(qa (condition-case error (let ((o (qa307-detached))) (list (overlay-put o 'y 2) (overlay-get o 'x) (overlay-get o 'y) (overlay-properties o))) (error error))
    '(2 1 2 (y 2 x 1)))

;; detached plist copy
(qa (condition-case error (let* ((o (qa307-detached)) (properties (overlay-properties o))) (plist-put properties 'x 9) (list (overlay-get o 'x) properties)) (error error))
    '(1 (x 9)))

;; live plist copy
(qa (condition-case error (with-temp-buffer (let ((o (make-overlay 1 1))) (overlay-put o 'x 1) (let ((p (overlay-properties o))) (plist-put p 'x 9) (list (overlay-get o 'x) p)))) (error error))
    '(1 (x 9)))

;; delete again
(qa (condition-case error (let ((o (qa307-detached))) (list (delete-overlay o) (overlay-get o 'x) (overlay-properties o))) (error error))
    '(nil 1 (x 1)))

;; revive in another buffer
(qa (condition-case error (with-temp-buffer (insert "abcd") (let ((o (qa307-detached))) (overlay-put o 'y 2) (list (eq (move-overlay o 2 4 (current-buffer)) o) (eq (overlay-buffer o) (current-buffer)) (overlay-start o) (overlay-end o) (overlay-get o 'x) (overlay-get o 'y)))) (error error))
    '(t t 2 4 1 2))

;; default buffer revival
(qa (condition-case error (with-temp-buffer (insert "abcd") (let ((o (qa307-detached))) (move-overlay o 1 3) (list (eq (overlay-buffer o) (current-buffer)) (overlay-start o) (overlay-end o)))) (error error))
    '(t 1 3))

;; copy detached
(qa (condition-case error (let* ((o (qa307-detached)) (copy (copy-overlay o))) (list (not (eq o copy)) (overlayp copy) (overlay-buffer copy) (overlay-start copy) (overlay-get copy 'x))) (error error))
    '(t t nil nil 1))

;; copy property independence
(qa (condition-case error (let* ((o (qa307-detached)) (copy (copy-overlay o))) (overlay-put copy 'x 8) (overlay-put o 'y 2) (list (overlay-get o 'x) (overlay-get copy 'x) (overlay-get copy 'y))) (error error))
    '(1 8 nil))

;; revive copy independently
(qa (condition-case error (with-temp-buffer (insert "abcd") (let* ((o (qa307-detached)) (copy (copy-overlay o))) (move-overlay copy 2 4) (list (overlay-buffer o) (overlay-get o 'x) (eq (overlay-buffer copy) (current-buffer)) (overlay-start copy) (overlay-get copy 'x)))) (error error))
    '(nil 1 t 2 1))

;; opaque detached printing
(qa (condition-case error (prin1-to-string (qa307-detached)) (error error))
    '"#<overlay in no buffer>")

;; canonical identity after revival
(qa (condition-case error (with-temp-buffer (insert "abcd") (let ((o (qa307-detached))) (move-overlay o 1 3) (and (memq o (overlays-at 2)) t))) (error error))
    't)

;; category after kill
(qa (condition-case error (let ((o (qa307-detached))) (put 'qa307-category 'z 44) (overlay-put o 'category 'qa307-category) (overlay-get o 'z)) (error error))
    '44)

;; GC preserves property references
(qa (condition-case error (let ((o (qa307-detached)) (value (copy-sequence "held"))) (overlay-put o 'value value) (garbage-collect) (list (overlayp o) (eq (overlay-get o 'value) value))) (error error))
    '(t t))

;; non-symbol property
(qa (condition-case error (let ((o (qa307-detached)) (key (list 'key))) (overlay-put o key 9) (overlay-get o key)) (error error))
    '9)

;; old owner killed first
(qa (condition-case error (qa307-cross-buffer t) (error error))
    '(t nil nil 1 2))

;; old owner killed last
(qa (condition-case error (qa307-cross-buffer nil) (error error))
    '(t nil nil 1 2))

;; insertion flags nil/nil
(qa (condition-case error (qa307-flags nil nil) (error error))
    '(2 5 1))

;; insertion flags nil/t
(qa (condition-case error (qa307-flags nil t) (error error))
    '(2 6 1))

;; insertion flags t/nil
(qa (condition-case error (qa307-flags t nil) (error error))
    '(3 5 1))

;; insertion flags t/t
(qa (condition-case error (qa307-flags t t) (error error))
    '(3 6 1))

;; Overlay display must leave ordinary vectors in their readable form.
(qa (prin1-to-string [1 2]) "[1 2]")
(qa (prin1-to-string [overlay 0 0]) "[overlay 0 0]")

(princ (qa--done))
