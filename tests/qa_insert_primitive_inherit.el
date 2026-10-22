;;; qa_insert_primitive_inherit.el --- Primitive insertion inheritance -*- lexical-binding: t; -*-
;; Properties are installed after before hooks and visible to after hooks.

(defun qa316-insert (function code count inherit properties position &optional before unibyte)
  (with-temp-buffer
    (when unibyte (set-buffer-multibyte nil))
    (insert (apply #'propertize "a" properties))
    (goto-char position)
    (let ((observed nil)
          (before-change-functions
           (and before (list (lambda (&rest _) (put-text-property 1 2 'face 'italic))))))
      (let ((after-change-functions
             (list (lambda (begin end old)
                     (push (list begin end old (get-text-property begin 'face)) observed)))))
        (list (funcall function code count inherit)
              (string-to-list (buffer-string))
              (mapcar (lambda (position) (get-text-property position 'face))
                      (number-sequence 1 (1- (point-max))))
              (nreverse observed))))))

;; insert-char rear inheritance
(qa (qa316-insert #'insert-char 32 2 t '(face bold) 2)
    '(nil (97 32 32) (bold bold bold) ((2 4 0 bold))))

;; insert-char nil does not inherit
(qa (qa316-insert #'insert-char 32 2 nil '(face bold) 2)
    '(nil (97 32 32) (bold nil nil) ((2 4 0 nil))))

;; insert-char front sticky inheritance
(qa (qa316-insert #'insert-char 32 2 t '(face bold front-sticky (face)) 1)
    '(nil (32 32 97) (bold bold bold) ((1 3 0 bold))))

;; insert-char front default is nonsticky
(qa (qa316-insert #'insert-char 32 2 t '(face bold) 1)
    '(nil (32 32 97) (nil nil bold) ((1 3 0 nil))))

;; insert-char rear nonsticky respected
(qa (qa316-insert #'insert-char 32 2 t '(face bold rear-nonsticky (face)) 2)
    '(nil (97 32 32) (bold nil nil) ((2 4 0 nil))))

;; insert-char before hook changes inherited face
(qa (qa316-insert #'insert-char 32 1 t '(face bold) 2 t)
    '(nil (97 32) (italic italic) ((2 3 0 italic))))

;; insert-char zero count
(qa (qa316-insert #'insert-char 32 0 t '(face bold) 2)
    '(nil (97) (bold) nil))

;; insert-char multiple characters
(qa (qa316-insert #'insert-char 32 3 t '(face bold) 2)
    '(nil (97 32 32 32) (bold bold bold bold) ((2 5 0 bold))))

;; insert-char unibyte buffer
(qa (qa316-insert #'insert-char 32 2 t '(face bold) 2 nil t)
    '(nil (97 32 32) (bold bold bold) ((2 4 0 bold))))

;; insert-char any non-nil inherit value
(qa (qa316-insert #'insert-char 32 1 'yes '(face bold) 2)
    '(nil (97 32) (bold bold) ((2 3 0 bold))))

;; insert-byte rear inheritance
(qa (qa316-insert #'insert-byte 32 2 t '(face bold) 2)
    '(nil (97 32 32) (bold bold bold) ((2 4 0 bold))))

;; insert-byte nil does not inherit
(qa (qa316-insert #'insert-byte 32 2 nil '(face bold) 2)
    '(nil (97 32 32) (bold nil nil) ((2 4 0 nil))))

;; insert-byte front sticky inheritance
(qa (qa316-insert #'insert-byte 32 2 t '(face bold front-sticky (face)) 1)
    '(nil (32 32 97) (bold bold bold) ((1 3 0 bold))))

;; insert-byte front default is nonsticky
(qa (qa316-insert #'insert-byte 32 2 t '(face bold) 1)
    '(nil (32 32 97) (nil nil bold) ((1 3 0 nil))))

;; insert-byte rear nonsticky respected
(qa (qa316-insert #'insert-byte 32 2 t '(face bold rear-nonsticky (face)) 2)
    '(nil (97 32 32) (bold nil nil) ((2 4 0 nil))))

;; insert-byte before hook changes inherited face
(qa (qa316-insert #'insert-byte 32 1 t '(face bold) 2 t)
    '(nil (97 32) (italic italic) ((2 3 0 italic))))

;; insert-byte zero count
(qa (qa316-insert #'insert-byte 32 0 t '(face bold) 2)
    '(nil (97) (bold) nil))

;; insert-byte multiple characters
(qa (qa316-insert #'insert-byte 32 3 t '(face bold) 2)
    '(nil (97 32 32 32) (bold bold bold bold) ((2 5 0 bold))))

;; insert-byte unibyte buffer
(qa (qa316-insert #'insert-byte 32 2 t '(face bold) 2 nil t)
    '(nil (97 32 32) (bold bold bold) ((2 4 0 bold))))

;; insert-byte any non-nil inherit value
(qa (qa316-insert #'insert-byte 32 1 'yes '(face bold) 2)
    '(nil (97 32) (bold bold) ((2 3 0 bold))))

;; Unicode character inherits
(qa (qa316-insert #'insert-char 233 1 t '(face bold) 2)
    '(nil (97 233) (bold bold) ((2 3 0 bold))))

;; raw byte in multibyte buffer
(qa (qa316-insert #'insert-byte 255 1 t '(face bold) 2)
    '(nil (97 4194303) (bold bold) ((2 3 0 bold))))

;; raw byte in unibyte buffer
(qa (qa316-insert #'insert-byte 255 1 t '(face bold) 2 nil t)
    '(nil (97 255) (bold bold) ((2 3 0 bold))))

;; indent hook sees inherited face
(qa (with-temp-buffer (insert (propertize "a" 'face 'bold)) (let ((observed nil)) (let ((after-change-functions (list (lambda (begin _end _old) (setq observed (get-text-property begin 'face)))))) (indent-to 3) observed)))
    'bold)

;; The shared primitive must retain buffer-level write protection.
(qa (with-temp-buffer
      (setq buffer-read-only t)
      (condition-case error (insert-char 32 1 t) (error (car error))))
    'buffer-read-only)
(qa (with-temp-buffer
      (setq buffer-read-only t)
      (condition-case error (insert-byte 32 1 t) (error (car error))))
    'buffer-read-only)
(qa (with-temp-buffer
      (setq buffer-read-only t)
      (condition-case error (indent-to 3) (error (car error))))
    'buffer-read-only)

(princ (qa--done))
