;;; -*- lexical-binding: t; -*-
;; Issue #233: deleted undo strings carry original properties and byte kind.
(defvar qa233-kind)
(defvar qa233-point)
(defvar qa233-operation)
(defvar qa233-seen nil)

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (delete-region 2 3)
      (setq buffer-undo-list (primitive-undo 1 buffer-undo-list))
      (list (buffer-substring-no-properties 1 4)
            (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3))))
    '("abc" (bold bold bold)))

(dolist (qa233-kind '(plain styled gap nil-tree unibyte))
  (qa (with-temp-buffer
        (buffer-enable-undo)
        (when (eq qa233-kind 'unibyte) (set-buffer-multibyte nil))
        (insert "abcd")
        (cond ((eq qa233-kind 'styled) (put-text-property 1 5 'face 'bold))
              ((eq qa233-kind 'gap) (put-text-property 1 2 'face 'bold))
              ((eq qa233-kind 'nil-tree) (put-text-property 1 5 'face nil)))
        (setq buffer-undo-list nil)
        (delete-region 3 4)
        (let ((text (caar buffer-undo-list)))
          (list (substring-no-properties text) (multibyte-string-p text) (object-intervals text))))
      (list "c" (not (eq qa233-kind 'unibyte))
            (cond ((eq qa233-kind 'styled) '((0 1 (face bold))))
                  ((eq qa233-kind 'nil-tree) '((0 1 (face nil))))))))

;; Both deletion position signs use the property's original intervals.
(dolist (qa233-point '(1 3))
  (qa (with-temp-buffer
        (buffer-enable-undo)
        (insert (propertize "abc" 'face 'bold))
        (goto-char qa233-point)
        (setq buffer-undo-list nil)
        (delete-region 2 3)
        (let ((history buffer-undo-list))
          (setq buffer-undo-list nil)
          (primitive-undo 1 history))
        (list (buffer-substring-no-properties 1 4)
              (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3))
              (point)))
      (list "abc" '(bold bold bold) (if (= qa233-point 3) 3 2))))

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert "αβγδεζ")
      (put-text-property 1 4 'face 'bold)
      (put-text-property 3 6 'font-lock-face 'italic)
      (let ((payload (list 1)))
        (put-text-property 2 6 'qa233 payload)
        (setq buffer-undo-list nil)
        (delete-region 2 6)
        (let* ((history buffer-undo-list) (text (caar history))
               (snapshot (list (substring-no-properties text) (multibyte-string-p text)
                               (mapcar (lambda (p) (list (get-text-property p 'face text)
                                                         (get-text-property p 'font-lock-face text)
                                                         (eq (get-text-property p 'qa233 text) payload))) '(0 1 2 3)))))
          (setcar payload 2)
          (setq buffer-undo-list nil)
          (primitive-undo 1 history)
          (list snapshot (buffer-substring-no-properties 1 7)
                (mapcar (lambda (p) (list (get-text-property p 'face)
                                          (get-text-property p 'font-lock-face))) '(1 2 3 4 5 6))
                (mapcar (lambda (p) (eq (get-text-property p 'qa233) payload)) '(2 3 4 5))
                payload))))
    '(("βγδε" t ((bold nil t) (bold italic t) (nil italic t) (nil italic t)))
      "αβγδεζ" ((bold nil) (bold nil) (bold italic) (nil italic) (nil italic) (nil nil))
      (t t t t) (2)))

;; Before hooks can change the properties that actually get deleted.
(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (let ((before-change-functions
             (list (lambda (beg end)
                     (let ((buffer-undo-list t)) (put-text-property beg end 'face 'italic))))))
        (delete-region 2 3))
      (let ((text (caar buffer-undo-list)))
        (primitive-undo 1 buffer-undo-list)
        (list (get-text-property 0 'face text)
              (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))
    '(italic (bold italic bold)))

;; The snapshot must already be visible to after-change hooks.
(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (let ((qa233-seen nil)
            (after-change-functions (list (lambda (&rest _) (setq qa233-seen (get-text-property 0 'face (caar buffer-undo-list)))))))
        (delete-region 2 3)
        qa233-seen))
    'bold)

;; Undo creates redo entries; repeated cycles retain the same face values.
(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (delete-region 2 3)
      (let ((history buffer-undo-list) states)
        (dotimes (_ 3)
          (setq buffer-undo-list nil)
          (primitive-undo 1 history)
          (setq history buffer-undo-list)
          (push (list (buffer-substring-no-properties (point-min) (point-max))
                      (mapcar (lambda (p) (get-text-property p 'face))
                              (number-sequence (point-min) (1- (point-max))))) states))
        (nreverse states)))
    '(("abc" (bold bold bold)) ("ac" (bold bold)) ("abc" (bold bold bold))))

(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (buffer-enable-undo)
      (insert (propertize (unibyte-string 65 255 66) 'face 'bold))
      (setq buffer-undo-list nil)
      (delete-region 2 3)
      (let ((text (caar buffer-undo-list)))
        (primitive-undo 1 buffer-undo-list)
        (list (multibyte-string-p text) (string-to-list text)
              (get-text-property 0 'face text)
              (string-to-list (buffer-string))
              (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))
    '(nil (255) bold (65 255 66) (bold bold bold)))

;; Every editing entry point saves the same original properties.
(dolist (qa233-operation '(delete-region delete-char backward-delete-char
					 delete-and-extract-region erase-buffer replace-match
					 delete-rectangle clear-rectangle string-rectangle))
  (qa (with-temp-buffer
        (buffer-enable-undo)
        (insert (propertize "abc" 'face 'bold))
        (setq buffer-undo-list nil)
        (goto-char 2)
        (pcase qa233-operation
          ('delete-region (delete-region 2 3))
          ('delete-char (delete-char 1))
          ('backward-delete-char (goto-char 3) (backward-delete-char 1))
          ('delete-and-extract-region (delete-and-extract-region 2 3))
          ('erase-buffer (erase-buffer))
          ('replace-match (search-forward "b") (replace-match "Z"))
          ('delete-rectangle (delete-rectangle 2 3))
          ('clear-rectangle (clear-rectangle 2 3))
          ('string-rectangle (string-rectangle 2 3 "Z")))
        (primitive-undo 1 buffer-undo-list)
        (list (buffer-substring-no-properties 1 4)
              (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3))))
      '("abc" (bold bold bold))))

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "a bc" 'face 'bold))
      (setq buffer-undo-list nil)
      (delete-whitespace-rectangle 2 4)
      (primitive-undo 1 buffer-undo-list)
      (list (buffer-substring-no-properties 1 5)
            (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3 4))))
    '("a bc" (bold bold bold bold)))

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize " abc\n def" 'face 'bold))
      (setq buffer-undo-list nil)
      (indent-rigidly 1 (point-max) -1)
      (primitive-undo 1 buffer-undo-list)
      (list (buffer-substring-no-properties 1 (point-max))
            (mapcar (lambda (p) (get-text-property p 'face)) (number-sequence 1 9))))
    '(" abc\n def" (bold bold bold bold bold bold bold bold bold)))

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "a日本b" 'face 'bold))
      (setq buffer-undo-list nil)
      (encode-coding-region 2 4 'utf-8)
      (primitive-undo 1 buffer-undo-list)
      (list (buffer-substring-no-properties 1 (point-max))
            (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3 4))))
    '("a日本b" (bold bold bold bold)))

;; The base and indirect view share history and properties.
(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (let ((base (current-buffer))
            (indirect (make-indirect-buffer (current-buffer) "qa233-indirect")))
        (unwind-protect
            (progn
              (with-current-buffer indirect (delete-region 2 3))
              (with-current-buffer base (primitive-undo 1 buffer-undo-list))
              (with-current-buffer indirect
                (list (buffer-substring-no-properties 1 4)
                      (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))
          (kill-buffer indirect))))
    '("abc" (bold bold bold)))

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (delete-region 2 3)
      (let ((history buffer-undo-list))
        (let ((buffer-undo-list t)) (put-text-property 1 3 'face 'italic))
        (primitive-undo 1 history)
        (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3))))
    '(italic bold italic))

;; The value survives solely through the undo string's property intervals.
(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert "abc")
      (put-text-property 2 3 'qa233 (list 'retained))
      (setq buffer-undo-list nil)
      (delete-region 2 3)
      (garbage-collect)
      (get-text-property 0 'qa233 (caar buffer-undo-list)))
    '(retained))

(qa (with-temp-buffer
      (insert (propertize "abc" 'face 'bold))
      (delete-region 2 3)
      (list buffer-undo-list (buffer-substring-no-properties 1 3)))
    '(t "ac"))

(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (buffer-enable-undo)
      (insert (propertize "abc" 'face 'bold))
      (setq buffer-undo-list nil)
      (let ((base (current-buffer))
            (indirect (make-indirect-buffer (current-buffer) "qa233-unibyte-indirect")))
        (unwind-protect
            (let ((kind (with-current-buffer indirect
                          (delete-region 2 3)
                          (multibyte-string-p (caar buffer-undo-list)))))
              (with-current-buffer base
                (primitive-undo 1 buffer-undo-list)
                (list kind (buffer-substring-no-properties 1 4)
                      (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))
          (kill-buffer indirect))))
    '(nil "abc" (bold bold bold)))

(qa (with-temp-buffer
      (buffer-enable-undo)
      (insert (propertize "012345" 'face 'bold))
      (setq buffer-undo-list nil)
      (narrow-to-region 2 6)
      (delete-region 3 5)
      (primitive-undo 1 buffer-undo-list)
      (widen)
      (list (buffer-substring-no-properties 1 7)
            (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3 4 5 6))))
    '("012345" (bold bold bold bold bold bold)))

(princ (qa--done)) (terpri)
