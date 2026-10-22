;;; qa_overlay_arguments.el --- Overlay APIs require registered objects -*- lexical-binding: t; -*-
;; Every getter/mutator rejects ordinary look-alike vectors with the original datum.

(defun qa308-rejections (value)
  (list (overlayp value)
        (mapcar
         (lambda (operation)
           (condition-case error (funcall operation value)
             (wrong-type-argument
              (list (car error) (cadr error) (eq (caddr error) value)))))
         (list #'overlay-start #'overlay-end #'overlay-buffer
               (lambda (o) (overlay-get o 'x))
               (lambda (o) (overlay-put o 'x 1))
               #'overlay-properties #'delete-overlay
               (lambda (o) (move-overlay o 1 1)) #'copy-overlay))))

(defconst qa308-errors '((wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t)))

;; reject nil
(qa (qa308-rejections nil)
    (list nil qa308-errors))

;; reject 42
(qa (qa308-rejections 42)
    (list nil qa308-errors))

;; reject "fake"
(qa (qa308-rejections "fake")
    (list nil qa308-errors))

;; reject []
(qa (qa308-rejections [])
    (list nil qa308-errors))

;; reject [overlay]
(qa (qa308-rejections [overlay])
    (list nil qa308-errors))

;; reject [overlay 0 0]
(qa (qa308-rejections [overlay 0 0])
    (list nil qa308-errors))

;; reject [overlay -1 -1]
(qa (qa308-rejections [overlay -1 -1])
    (list nil qa308-errors))

;; reject [overlay 123456 0]
(qa (qa308-rejections [overlay 123456 0])
    (list nil qa308-errors))

;; reject [other 0 0]
(qa (qa308-rejections [other 0 0])
    (list nil qa308-errors))

;; reject [overlay 0 0 nil nil nil nil]
(qa (qa308-rejections [overlay 0 0 nil nil nil nil])
    (list nil qa308-errors))

;; reject (record 'overlay 0 0)
(qa (qa308-rejections (record 'overlay 0 0))
    (list nil qa308-errors))

;; reject (copy-sequence [overlay 0 0])
(qa (qa308-rejections (copy-sequence [overlay 0 0]))
    (list nil qa308-errors))

;; genuine deleted overlay
(qa (with-temp-buffer (let ((o (make-overlay 1 1))) (overlay-put o 'x 9) (delete-overlay o) (list (overlayp o) (overlay-get o 'x) (overlay-start o))))
    '(t 9 nil))

;; genuine killed overlay
(qa (let ((o (with-temp-buffer (let ((o (make-overlay 1 1))) (overlay-put o 'x 9) o)))) (list (overlayp o) (overlay-start o) (overlay-put o 'y 2) (overlay-get o 'x) (overlay-get o 'y)))
    '(t nil 2 9 2))

;; ordinary vector operations
(qa (list (vectorp [overlay 0 0]) (length [overlay 0 0]) (aref [overlay 0 0] 0) (prin1-to-string [overlay 0 0]))
    '(t 3 overlay "[overlay 0 0]"))

;; forged access cannot modify real overlay
(qa (let ((o (make-overlay (point-min) (point-min)))) (unwind-protect (progn (overlay-put o 'x 9) (list (qa308-rejections [overlay 0 0]) (eq (overlay-buffer o) (current-buffer)) (overlay-get o 'x))) (delete-overlay o)))
    '((nil ((wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t) (wrong-type-argument overlayp t))) t 9))

(princ (qa--done))
