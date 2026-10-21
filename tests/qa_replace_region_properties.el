;;; -*- lexical-binding: t; -*-
;; Issue #236: preserve SOURCE properties only on inserted diff runs.
(defun qa236-snapshot ()
  (list (buffer-substring-no-properties (point-min) (point-max))
        (mapcar (lambda (p) (list (get-text-property p 'face)
                                 (get-text-property p 'font-lock-face)
                                 (get-text-property p 'qa236)))
                (number-sequence (point-min) (1- (point-max))))))
(defun qa236-source (kind)
  (let* ((payload (list 'shared))
         (text (propertize "<aXcdYf>" 'face 'bold 'qa236 payload))
         (source (generate-new-buffer " *qa236-source*")))
    (put-text-property 2 4 'font-lock-face 'underline text)
    (unwind-protect
        (progn
          (with-current-buffer source (insert text))
          (with-temp-buffer
            (insert (propertize "abcdef" 'face 'italic))
            (replace-region-contents
             1 7
             (cond ((eq kind 'string) (substring text 1 7))
                   ((eq kind 'buffer)
                    (with-current-buffer source (narrow-to-region 2 8)) source)
                   ((eq kind 'vector) (vector source 2 8))
                   ((eq kind 'callback-string) (lambda () (substring text 1 7)))
                   ((eq kind 'callback-buffer)
                    (lambda () (with-current-buffer source (narrow-to-region 2 8)) source))
                   ((eq kind 'callback-vector) (lambda () (vector source 2 8)))))
            (let ((before (mapcar (lambda (p) (eq (get-text-property p 'qa236) payload)) '(2 5))))
              ;; Intervals are independent; property values retain identity.
              (put-text-property 2 3 'face 'shadow text)
              (with-current-buffer source (put-text-property 3 4 'face 'shadow))
              (garbage-collect)
              (list (qa236-snapshot) before
                    (eq (get-text-property 2 'qa236) payload)
                    (eq (get-text-property 5 'qa236) payload)))))
      (kill-buffer source))))
(defvar qa236-seen nil)

(qa
 (with-temp-buffer
   (insert "abc")
   (replace-region-contents 1 4 (propertize "aXc" 'face 'bold))
   (qa236-snapshot))
 '("aXc" ((nil nil nil) (bold nil nil) (nil nil nil))))

(qa (qa236-source 'string)
    '(("aXcdYf"
       ((italic nil nil) (bold underline (shared)) (italic nil nil)
	(italic nil nil) (bold nil (shared)) (italic nil nil)))
      (t t) t t))

(qa (qa236-source 'buffer)
    '(("aXcdYf"
       ((italic nil nil) (bold underline (shared)) (italic nil nil)
	(italic nil nil) (bold nil (shared)) (italic nil nil)))
      (t t) t t))

(qa (qa236-source 'vector)
    '(("aXcdYf"
       ((italic nil nil) (bold underline (shared)) (italic nil nil)
	(italic nil nil) (bold nil (shared)) (italic nil nil)))
      (t t) t t))

(qa (qa236-source 'callback-string)
    '(("aXcdYf"
       ((italic nil nil) (bold underline (shared)) (italic nil nil)
	(italic nil nil) (bold nil (shared)) (italic nil nil)))
      (t t) t t))

(qa (qa236-source 'callback-buffer)
    '(("aXcdYf"
       ((italic nil nil) (bold underline (shared)) (italic nil nil)
	(italic nil nil) (bold nil (shared)) (italic nil nil)))
      (t t) t t))

(qa (qa236-source 'callback-vector)
    '(("aXcdYf"
       ((italic nil nil) (bold underline (shared)) (italic nil nil)
	(italic nil nil) (bold nil (shared)) (italic nil nil)))
      (t t) t t))

(qa
 (with-temp-buffer
   (insert (propertize "abc" 'face 'italic)) (goto-char 2)
   (buffer-enable-undo) (set-buffer-modified-p nil)
   (let ((tick (buffer-modified-tick)))
     (replace-region-contents 1 4 (propertize "abc" 'face 'bold))
     (list (qa236-snapshot) (point) (buffer-modified-p)
	   (= tick (buffer-modified-tick)) buffer-undo-list)))
 '(("abc" ((italic nil nil) (italic nil nil) (italic nil nil))) 2 nil
   t nil))

(qa
 (with-temp-buffer
   (let ((source (propertize "XYZ" 'face 'bold)))
     (put-text-property 1 2 'face nil source)
     (replace-region-contents 1 1 source)
     (list (qa236-snapshot) (object-intervals (buffer-string)))))
 '(("XYZ" ((bold nil nil) (nil nil nil) (bold nil nil)))
   ((0 1 (face bold)) (1 2 (face nil)) (2 3 (face bold)))))

(qa
 (with-temp-buffer
   (insert (propertize "abc" 'face 'italic))
   (replace-region-contents 1 4 "aXYZc") (qa236-snapshot))
 '("aXYZc"
   ((italic nil nil) (nil nil nil) (nil nil nil) (nil nil nil)
    (italic nil nil))))

(qa
 (with-temp-buffer
   (insert "abcdef")
   (let ((source (copy-sequence "aXYdeZ")))
     (put-text-property 1 2 'face 'bold source)
     (put-text-property 2 3 'font-lock-face 'underline source)
     (put-text-property 5 6 'face 'italic source)
     (replace-region-contents 1 7 source) (qa236-snapshot)))
 '("aXYdeZ"
   ((nil nil nil) (bold nil nil) (nil underline nil) (nil nil nil)
    (nil nil nil) (italic nil nil))))

(qa
 (with-temp-buffer
   (insert (propertize "abcdef" 'face 'italic))
   (let ((source (propertize "abXdeY" 'face 'bold)))
     (put-text-property 2 3 'face nil source)
     (replace-region-contents 1 7 source) (qa236-snapshot)))
 '("abXdeY"
   ((italic nil nil) (italic nil nil) (nil nil nil) (italic nil nil)
    (italic nil nil) (bold nil nil))))

(qa
 (with-temp-buffer
   (insert "αβγδεζ")
   (replace-region-contents 1 7 (propertize "αΩγδΨζ" 'face 'bold))
   (qa236-snapshot))
 '("αΩγδΨζ"
   ((nil nil nil) (bold nil nil) (nil nil nil) (nil nil nil)
    (bold nil nil) (nil nil nil))))

(qa
 (with-temp-buffer
   (set-buffer-multibyte nil) (insert "abcdef")
   (replace-region-contents 1 7 (propertize "aXcdYf" 'face 'bold))
   (list (multibyte-string-p (buffer-string)) (qa236-snapshot)))
 '(nil
   ("aXcdYf"
    ((nil nil nil) (bold nil nil) (nil nil nil) (nil nil nil)
     (bold nil nil) (nil nil nil)))))

(qa
 (with-temp-buffer
   (set-buffer-multibyte t) (insert "abcdef")
   (replace-region-contents 1 7 (propertize "aXcdYf" 'face 'bold))
   (list (multibyte-string-p (buffer-string)) (qa236-snapshot)))
 '(t
   ("aXcdYf"
    ((nil nil nil) (bold nil nil) (nil nil nil) (nil nil nil)
     (bold nil nil) (nil nil nil)))))

(qa
 (with-temp-buffer
   (insert "abc")
   (let
       ((result
	 (replace-region-contents 1 4 (propertize "aXc" 'face 'bold) 0
				  nil)))
     (list result (qa236-snapshot))))
 '(t ("aXc" ((bold nil nil) (bold nil nil) (bold nil nil)))))

(qa
 (with-temp-buffer
   (insert "abc")
   (let
       ((result
	 (replace-region-contents 1 4 (propertize "aXc" 'face 'bold)
				  nil 0)))
     (list result (qa236-snapshot))))
 '(t ("aXc" ((bold nil nil) (bold nil nil) (bold nil nil)))))

(qa
 (with-temp-buffer
   (insert "abc")
   (let
       ((result
	 (replace-region-contents 1 4 (propertize "aXc" 'face 'bold)
				  -1 nil)))
     (list result (qa236-snapshot))))
 '(nil ("aXc" ((bold nil nil) (bold nil nil) (bold nil nil)))))

(qa
 (with-temp-buffer
   (insert "abc")
   (let
       ((after-change-functions
	 (list
	  (lambda (&rest _args)
	    (setq qa236-seen (get-text-property 2 'face)))))
	(qa236-seen nil))
     (replace-region-contents 1 4 (propertize "aXc" 'face 'bold))
     qa236-seen))
 'bold)

(qa
 (with-temp-buffer
   (insert (propertize "abc" 'face 'italic)) (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (replace-region-contents 1 4 (propertize "aXc" 'face 'bold))
   (let ((changed (qa236-snapshot)))
     (setq buffer-undo-list (primitive-undo 1 buffer-undo-list))
     (list changed (qa236-snapshot))))
 '(("aXc" ((italic nil nil) (bold nil nil) (italic nil nil)))
   ("abc" ((italic nil nil) (italic nil nil) (italic nil nil)))))


(princ (qa--done)) (terpri)
