;;; qa_erase_buffer_widen.el --- Widen before erasing and preserve hook results -*- lexical-binding: t; -*-

(defun qa-erase-state ()
  (list (save-restriction (widen) (buffer-substring-no-properties (point-min) (point-max)))
        (buffer-size) (point-min) (point-max) (point) (buffer-narrowed-p)))
(defun qa-erase-run (narrow before after readonly &optional empty)
  (with-temp-buffer
    (unless empty (insert "abcdef") (goto-char 4))
    (when (and narrow (not empty)) (narrow-to-region 2 5))
    (setq buffer-read-only readonly)
    (let* ((events nil)
           (before-change-functions
            (and before
                 (list (lambda (begin end)
                         (push (list 'before begin end (point-min) (point-max)) events)
                         (cond ((eq before 'insert) (save-excursion (goto-char 1) (insert "P")))
                               ((eq before 'error) (error "qa-erase-stop")))))))
           (after-change-functions
            (and after
                 (list (lambda (begin end old)
                         (push (list 'after begin end old) events)
                         (cond ((eq after 'insert) (insert "X"))
                               ((eq after 'narrow) (insert "XY") (narrow-to-region 2 3))
                               ((eq after 'error) (error "qa-erase-stop"))))))))
      (list (condition-case error (erase-buffer) (error (car error)))
            (qa-erase-state) (nreverse events)))))
(defun qa-erase-switch ()
  (with-temp-buffer
    (insert "abcdef") (narrow-to-region 2 5)
    (let ((original (current-buffer))
          (other (generate-new-buffer " *qa-erase-other*")))
      (unwind-protect
          (progn
            (with-current-buffer other
              (insert "SAFE") (goto-char 3) (narrow-to-region 2 4))
            (let ((after-change-functions (list (lambda (&rest _) (set-buffer other)))))
              (let ((result (erase-buffer)))
                (list result (qa-erase-state)
                      (with-current-buffer original (qa-erase-state))))))
        (set-buffer original)
        (kill-buffer other)))))

(qa (qa-erase-run t nil nil nil)
    '(nil ("" 0 1 1 1 nil) nil))

(qa (qa-erase-run nil nil nil nil)
    '(nil ("" 0 1 1 1 nil) nil))

(qa (qa-erase-run t nil nil t)
    '(buffer-read-only ("abcdef" 6 1 7 4 nil) nil))

(qa (qa-erase-run nil nil nil t t)
    '(nil ("" 0 1 1 1 nil) nil))

(qa (qa-erase-run t 'observe 'observe nil)
    '(nil ("" 0 1 1 1 nil) ((before 1 7 1 7) (after 1 1 6))))

(qa (qa-erase-run t 'insert 'observe nil)
    '(nil ("f" 1 1 2 1 nil) ((before 1 7 1 7) (after 1 1 6))))

(qa (qa-erase-run t nil 'insert nil)
    '(nil ("X" 1 1 2 2 nil) ((after 1 1 6))))

(qa (qa-erase-run t nil 'narrow nil)
    '(nil ("XY" 2 2 3 3 t) ((after 1 1 6))))

(qa (qa-erase-run t 'error 'observe nil)
    '(error ("abcdef" 6 1 7 4 nil) ((before 1 7 1 7))))

(qa (qa-erase-run t nil 'error nil)
    '(error ("" 0 1 1 1 nil) ((after 1 1 6))))

(qa (qa-erase-switch)
    '(nil ("SAFE" 4 2 4 3 t) ("" 0 1 1 1 nil)))

;; Erasing also discards labeled restrictions before a read-only error.
(qa (with-temp-buffer
  (insert "abcdef")
  (narrow-to-region 2 6)
  (internal--labeled-narrow-to-region 3 5 'qa-erase-label)
  (setq buffer-read-only t)
  (let ((result (condition-case error (erase-buffer) (error (car error)))))
    (list result (list (point-min) (point-max))
          (progn (internal--labeled-widen 'qa-erase-label)
                 (list (point-min) (point-max))))))
    '(buffer-read-only (1 7) (1 7)))

(princ (qa--done))
