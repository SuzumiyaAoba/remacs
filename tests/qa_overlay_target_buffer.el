;;; qa_overlay_target_buffer.el --- Overlay target validation -*- lexical-binding: t; -*-
;; Dead target errors precede position decoding and leave the source unchanged.

(defun qa311-make-dead (begin end)
  (let ((dead (generate-new-buffer " *qa311-dead*")))
    (kill-buffer dead)
    (condition-case error (make-overlay begin end dead) (error error))))

(defun qa311-move-dead (begin end &optional evaporate detached)
  (with-temp-buffer
    (insert "abcdef")
    (let ((dead (generate-new-buffer " *qa311-dead*")) (o (make-overlay 1 3)))
      (kill-buffer dead)
      (overlay-put o 'x 1)
      (when evaporate (overlay-put o 'evaporate t))
      (when detached (delete-overlay o))
      (list (condition-case error (move-overlay o begin end dead) (error error))
            (eq (overlay-buffer o) (current-buffer))
            (overlay-start o) (overlay-end o) (overlay-get o 'x)))))

(defun qa311-default (explicit-nil detached)
  (with-temp-buffer
    (insert "abcdef")
    (let ((source (current-buffer)) (o (make-overlay 1 3))
          (other (generate-new-buffer " *qa311-other*")))
      (unwind-protect
          (progn
            (when detached (delete-overlay o))
            (with-current-buffer other
              (insert "xy")
              (if explicit-nil (move-overlay o 2 4 nil) (move-overlay o 2 4))
              (list (eq (overlay-buffer o) source) (eq (overlay-buffer o) other)
                    (overlay-start o) (overlay-end o))))
        (kill-buffer other)))))

;; dead target creation
(qa (qa311-make-dead 1 1)
    '(error "Attempt to create overlay in a dead buffer"))

;; liveness before bad positions
(qa (qa311-make-dead "bad" nil)
    '(error "Attempt to create overlay in a dead buffer"))

;; liveness before detached marker
(qa (qa311-make-dead (make-marker) 1)
    '(error "Attempt to create overlay in a dead buffer"))

;; dead move target
(qa (qa311-move-dead 1 3)
    '((error "Attempt to move overlay to a dead buffer") t 1 3 1))

;; move liveness before positions
(qa (qa311-move-dead "bad" nil)
    '((error "Attempt to move overlay to a dead buffer") t 1 3 1))

;; invalid target cannot evaporate source
(qa (qa311-move-dead 1 1 t)
    '((error "Attempt to move overlay to a dead buffer") t 1 3 1))

;; detached source still validates target
(qa (qa311-move-dead 1 1 nil t)
    '((error "Attempt to move overlay to a dead buffer") nil nil nil 1))

;; names are not buffer arguments
(qa (condition-case error (make-overlay 1 1 "*scratch*") (error error))
    '(wrong-type-argument bufferp "*scratch*"))

;; move checks buffer type
(qa (with-temp-buffer (let ((o (make-overlay 1 1))) (condition-case error (move-overlay o 1 1 42) (error error))))
    '(wrong-type-argument bufferp 42))

;; overlay type precedes target liveness
(qa (let ((dead (generate-new-buffer " *qa311-dead*"))) (kill-buffer dead) (condition-case error (move-overlay [overlay 0 0] 1 1 dead) (error error)))
    '(wrong-type-argument overlayp [overlay 0 0]))

;; evaluation continues after error
(qa (with-temp-buffer (list (car (qa311-make-dead 1 1)) (overlayp (make-overlay 1 1))))
    '(error t))

;; omitted buffer retains attachment
(qa (qa311-default nil nil)
    '(t nil 2 4))

;; nil buffer retains attachment
(qa (qa311-default t nil)
    '(t nil 2 4))

;; deleted overlay uses current buffer
(qa (qa311-default nil t)
    '(nil t 2 3))

;; killed overlay uses current buffer
(qa (with-temp-buffer (insert "xy") (let ((o (with-temp-buffer (insert "abcdef") (make-overlay 1 3)))) (move-overlay o 2 4) (list (eq (overlay-buffer o) (current-buffer)) (overlay-start o) (overlay-end o))))
    '(t 2 3))

(princ (qa--done))
