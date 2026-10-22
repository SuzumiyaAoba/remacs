;;; qa_extract_metadata.el --- Preserve deleted text metadata -*- lexical-binding: t; -*-

(defun qa325-extract (unibyte text begin end &optional before after)
  (with-temp-buffer
    (when unibyte (set-buffer-multibyte nil))
    (insert (propertize text 'face 'bold 'audit 42))
    (let* ((before-change-functions
            (and before
                 (list (lambda (begin end)
                         (if (eq before 'prefix)
                             (save-excursion (goto-char 1) (insert "P"))
                           (put-text-property begin end 'face 'italic))))))
           (after-change-functions
            (and after
                 (list (lambda (&rest _)
                         (if (eq after 'mode)
                             (set-buffer-multibyte nil)
                           (put-text-property (point-min) (point-max) 'face 'underline))))))
           (result (delete-and-extract-region begin end)))
      (list (multibyte-string-p result) (string-to-list result) (string-bytes result)
            (mapcar (lambda (p) (get-text-property p 'face result))
                    (number-sequence 0 (1- (length result))))
            (mapcar (lambda (p) (get-text-property p 'audit result))
                    (number-sequence 0 (1- (length result))))
            (string-to-list (buffer-string))))))

;; multibyte ASCII
(qa (qa325-extract nil "abc" 1 3)
    '(t (97 98) 2 (bold bold) (42 42) (99)))

;; unibyte ASCII
(qa (qa325-extract t "abc" 1 3)
    '(nil (97 98) 2 (bold bold) (42 42) (99)))

;; unibyte high bytes
(qa (qa325-extract t (unibyte-string 255 65 66) 1 3)
    '(nil (255 65) 2 (bold bold) (42 42) (66)))

;; multibyte raw bytes
(qa (qa325-extract nil (unibyte-string 255 65 66) 1 3)
    '(t (4194303 65) 3 (bold bold) (42 42) (66)))

;; Unicode
(qa (qa325-extract nil "日本abc" 1 3)
    '(t (26085 26412) 6 (bold bold) (42 42) (97 98 99)))

;; ASCII prefix of Unicode buffer
(qa (qa325-extract nil "a日本" 1 2)
    '(t (97) 1 (bold) (42) (26085 26412)))

;; before hook updates properties
(qa (qa325-extract nil "abc" 1 3 'italic)
    '(t (97 98) 2 (italic italic) (42 42) (99)))

;; before hook updates unibyte properties
(qa (qa325-extract t (unibyte-string 255 65 66) 1 3 'italic)
    '(nil (255 65) 2 (italic italic) (42 42) (66)))

;; properties follow relocated deletion
(qa (qa325-extract nil "abcd" 2 4 'prefix)
    '(t (98 99) 2 (bold bold) (42 42) (80 97 100)))

;; after hook cannot replace extracted properties
(qa (qa325-extract nil "abc" 1 3 'italic 'properties)
    '(t (97 98) 2 (italic italic) (42 42) (99)))

;; after hook cannot replace extracted byte form
(qa (qa325-extract nil "abc" 1 3 nil 'mode)
    '(t (97 98) 2 (bold bold) (42 42) (99)))

;; empty multibyte extraction stays unibyte
(qa (qa325-extract nil "abc" 2 2)
    '(nil nil 0 nil nil (97 98 99)))

;; empty unibyte extraction
(qa (qa325-extract t "abc" 2 2)
    '(nil nil 0 nil nil (97 98 99)))

;; reversed region
(qa (qa325-extract nil "abc" 3 1)
    '(t (97 98) 2 (bold bold) (42 42) (99)))

;; whole buffer extraction
(qa (qa325-extract nil "日本" 1 3)
    '(t (26085 26412) 6 (bold bold) (42 42) nil))

(defun qa325-redirect (unibyte)
  (with-temp-buffer
    (when unibyte (set-buffer-multibyte nil))
    (insert (propertize "SAFE" 'face 'italic 'audit 99))
    (let ((outer (current-buffer))
          (target (generate-new-buffer " *qa325-target*")))
      (unwind-protect
          (let ((result
                 (with-current-buffer target
                   (unless unibyte (set-buffer-multibyte nil))
                   (insert (propertize "abcd" 'face 'bold 'audit 42))
                   (let ((before-change-functions
                          (list (lambda (&rest _) (set-buffer outer)))))
                     (delete-and-extract-region 2 3)))))
            (list (multibyte-string-p result) (string-to-list result)
                  (get-text-property 0 'face result) (get-text-property 0 'audit result)
                  (buffer-substring-no-properties (point-min) (point-max))))
        (kill-buffer target)))))

;; Snapshot the actual buffer selected by a live-target hook.
(qa (qa325-redirect nil)
    '(t (65) italic 99 "SFE"))

;; Snapshot the actual buffer selected by a live-target hook.
(qa (qa325-redirect t)
    '(nil (65) italic 99 "SFE"))

(qa (with-temp-buffer
      (insert "abc")
      (let* ((before-change-functions (list (lambda (&rest _) (erase-buffer))))
             (result (delete-and-extract-region 1 3)))
        (list (multibyte-string-p result) (string-to-list result))))
    '(t nil))

(princ (qa--done))
