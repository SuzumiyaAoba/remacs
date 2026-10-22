;;; qa_newline_inheritance.el --- Newlines inherit sticky properties -*- lexical-binding: t; -*-

(defun qa317-newline (properties position count &optional before text)
  (with-temp-buffer
    (setq electric-indent-mode nil)
    (insert (apply #'propertize (or text "a") properties))
    (goto-char position)
    (let ((observed nil)
          (before-change-functions
           (and before (list (lambda (&rest _) (put-text-property 1 (point-max) 'face 'italic))))))
      (let ((after-change-functions
             (list (lambda (begin end old)
                     (push (list begin end old (get-text-property begin 'face)) observed)))))
        (list (newline count)
              (string-to-list (buffer-string))
              (mapcar (lambda (position) (get-text-property position 'face))
                      (number-sequence 1 (1- (point-max))))
              (point)
              (nreverse observed))))))

;; rear sticky default
(qa (qa317-newline '(face bold) 2 nil)
    '(nil (97 10) (bold bold) 3 ((2 3 0 bold))))

;; front sticky face
(qa (qa317-newline '(face bold front-sticky (face)) 1 nil)
    '(nil (10 97) (bold bold) 2 ((1 2 0 bold))))

;; front sticky all
(qa (qa317-newline '(face bold front-sticky t) 1 1)
    '(nil (10 97) (bold bold) 2 ((1 2 0 bold))))

;; front default nonsticky
(qa (qa317-newline '(face bold) 1 1)
    '(nil (10 97) (nil bold) 2 ((1 2 0 nil))))

;; rear nonsticky face
(qa (qa317-newline '(face bold rear-nonsticky (face)) 2 1)
    '(nil (97 10) (bold nil) 3 ((2 3 0 nil))))

;; rear nonsticky all
(qa (qa317-newline '(face bold rear-nonsticky t) 2 1)
    '(nil (97 10) (bold nil) 3 ((2 3 0 nil))))

;; multiple newlines
(qa (qa317-newline '(face bold) 2 2)
    '(nil (97 10 10) (bold bold bold) 4 ((2 4 0 bold))))

;; raw prefix count
(qa (qa317-newline '(face bold) 2 '(4))
    '(nil (97 10 10 10 10) (bold bold bold bold bold) 6 ((2 6 0 bold))))

;; zero count runs no hooks
(qa (with-temp-buffer
      (insert (propertize "a" 'face 'bold))
      (let ((observed nil))
        (let ((before-change-functions (list (lambda (&rest _) (push 'before observed))))
              (after-change-functions (list (lambda (&rest _) (push 'after observed)))))
          (list (newline 0) (string-to-list (buffer-string))
                (get-text-property 1 'face) observed))))
    '(nil (97) bold nil))

;; middle of interval
(qa (qa317-newline '(face bold) 2 1 nil "ab")
    '(nil (97 10 98) (bold bold bold) 3 ((2 3 0 bold))))

;; inherit after before hook
(qa (qa317-newline '(face bold) 2 1 t)
    '(nil (97 10) (italic italic) 3 ((2 3 0 italic))))

;; unpropertized insertion
(qa (qa317-newline nil 2 1)
    '(nil (97 10) (nil nil) 3 ((2 3 0 nil))))

;; Unicode characters around newline
(qa (qa317-newline '(face bold) 2 1 nil "日本")
    '(nil (26085 10 26412) (bold bold bold) 3 ((2 3 0 bold))))

;; buffer write protection
(qa (with-temp-buffer (setq buffer-read-only t) (condition-case error (newline 1) (error (car error))))
    'buffer-read-only)

;; inhibited write protection
(qa (with-temp-buffer (insert (propertize "a" 'face 'bold)) (setq buffer-read-only t) (let ((inhibit-read-only t) (electric-indent-mode nil)) (newline) (list (get-text-property 2 'face) (point))))
    '(bold 3))

;; negative numeric count
(qa (condition-case error (newline -1) (error (list (car error) (cadr error))))
    '(error "Repetition argument has to be non-negative"))

;; negative raw prefix
(qa (condition-case error (newline '-) (error (list (car error) (cadr error))))
    '(error "Repetition argument has to be non-negative"))

(princ (qa--done))
