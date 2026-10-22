;;; qa_empty_readonly.el --- Empty edits skip write protection -*- lexical-binding: t; -*-

(defun qa319-operation (form readonly &optional text text-readonly)
  (with-temp-buffer
    (insert (or text "abc"))
    (when text-readonly (put-text-property 1 (point-max) 'read-only t))
    (buffer-enable-undo)
    (setq buffer-undo-list nil buffer-read-only readonly)
    (set-buffer-modified-p nil)
    (goto-char (point-min))
    (let ((tick (buffer-modified-tick))
          (undo buffer-undo-list)
          (observed nil))
      (let ((before-change-functions (list (lambda (&rest _) (push 'before observed))))
            (after-change-functions (list (lambda (&rest _) (push 'after observed)))))
        (list (condition-case error (eval form t) (error (car error)))
              (string-to-list (buffer-string)) (point)
              (= tick (buffer-modified-tick)) (eq undo buffer-undo-list)
              (nreverse observed) (buffer-modified-p))))))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-char 65 0) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-char 65 0 t) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-char 65 -2) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-char 65 -2 t) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-byte 65 0) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-byte 65 0 t) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(delete-region 2 2) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(delete-and-extract-region 2 2) nil)
    '("" (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(delete-char 0) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert) nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert "") nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-and-inherit "") nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-before-markers "") nil)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only nil
(qa (qa319-operation '(insert-before-markers-and-inherit "") nil)
    '(nil (97 98 99) 1 t t nil nil))

;; erase genuinely empty buffer
(qa (qa319-operation '(erase-buffer) nil "")
    '(nil nil 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-char 65 0) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-char 65 0 t) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-char 65 -2) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-char 65 -2 t) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-byte 65 0) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-byte 65 0 t) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(delete-region 2 2) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(delete-and-extract-region 2 2) t)
    '("" (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(delete-char 0) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert) t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert "") t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-and-inherit "") t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-before-markers "") t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty operation, buffer-read-only t
(qa (qa319-operation '(insert-before-markers-and-inherit "") t)
    '(nil (97 98 99) 1 t t nil nil))

;; erase genuinely empty buffer
(qa (qa319-operation '(erase-buffer) t "")
    '(nil nil 1 t t nil nil))

;; nonempty edits retain write protection
(qa (qa319-operation '(insert-char 65 1) t)
    '(buffer-read-only (97 98 99) 1 t t nil nil))

;; nonempty edits retain write protection
(qa (qa319-operation '(delete-region 1 2) t)
    '(buffer-read-only (97 98 99) 1 t t nil nil))

;; nonempty edits retain write protection
(qa (qa319-operation '(delete-and-extract-region 1 2) t)
    '(buffer-read-only (97 98 99) 1 t t nil nil))

;; nonempty edits retain write protection
(qa (qa319-operation '(erase-buffer) t)
    '(buffer-read-only (97 98 99) 1 t t nil nil))

;; validate before no-op or read-only checks
(qa (qa319-operation '(insert-char -1 0) t)
    '(wrong-type-argument (97 98 99) 1 t t nil nil))

;; validate before no-op or read-only checks
(qa (qa319-operation '(insert-char 65 'bad) t)
    '(wrong-type-argument (97 98 99) 1 t t nil nil))

;; validate before no-op or read-only checks
(qa (qa319-operation '(delete-region 0 0) t)
    '(args-out-of-range (97 98 99) 1 t t nil nil))

;; validate before no-op or read-only checks
(qa (qa319-operation '(delete-and-extract-region nil nil) t)
    '(wrong-type-argument (97 98 99) 1 t t nil nil))

;; empty edit does not touch read-only text
(qa (qa319-operation '(insert-char 65 0 t) nil nil t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty edit does not touch read-only text
(qa (qa319-operation '(delete-region 2 2) nil nil t)
    '(nil (97 98 99) 1 t t nil nil))

;; empty edit does not touch read-only text
(qa (qa319-operation '(delete-and-extract-region 2 2) nil nil t)
    '("" (97 98 99) 1 t t nil nil))

(princ (qa--done))
