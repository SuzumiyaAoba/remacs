;;; -*- lexical-binding: t; -*-
;; Issue #241: attributes follow the transposed text, including explicit nil
;; values and overlapping plists; mutable property values retain identity.
(defun qa241-properties (regions leave)
  (with-temp-buffer
    (insert "ab--XYZ")
    (let ((payload (list 'shared)))
      (put-text-property 1 3 'face 'bold)
      (put-text-property 3 5 'face nil)
      (put-text-property 5 8 'face 'italic)
      (put-text-property 2 6 'font-lock-face 'underline)
      (put-text-property 1 8 'qa241 payload)
      (apply #'transpose-regions (append regions (list leave)))
      (garbage-collect)
      (list (buffer-substring-no-properties 1 8)
            (mapcar (lambda (p) (list (get-text-property p 'face)
                                     (get-text-property p 'font-lock-face)
                                     (eq (get-text-property p 'qa241) payload)
                                     (not (null (memq 'face (text-properties-at p))))))
                    (number-sequence 1 7))))))

(qa
 (with-temp-buffer
   (insert (propertize "ab" 'face 'bold) "--"
	   (propertize "XYZ" 'face 'italic))
   (transpose-regions 1 3 5 8)
   (list (buffer-substring-no-properties 1 8)
	 (mapcar (lambda (p) (get-text-property p 'face))
		 '(1 2 3 4 5 6 7))))
 '("XYZ--ab" (italic italic italic nil nil bold bold)))

(qa (qa241-properties '(1 3 5 8) nil)
    '("XYZ--ab"
      ((italic underline t t) (italic nil t t) (italic nil t t)
       (nil underline t t) (nil underline t t) (bold nil t t)
       (bold underline t t))))

(qa (qa241-properties '(1 3 5 8) t)
    '("XYZ--ab"
      ((italic underline t t) (italic nil t t) (italic nil t t)
       (nil underline t t) (nil underline t t) (bold nil t t)
       (bold underline t t))))

(qa (qa241-properties '(1 3 5 7) nil)
    '("XY--abZ"
      ((italic underline t t) (italic nil t t) (nil underline t t)
       (nil underline t t) (bold nil t t) (bold underline t t)
       (italic nil t t))))

(qa (qa241-properties '(1 3 5 7) t)
    '("XY--abZ"
      ((italic underline t t) (italic nil t t) (nil underline t t)
       (nil underline t t) (bold nil t t) (bold underline t t)
       (italic nil t t))))

(qa (qa241-properties '(1 3 3 8) nil)
    '("--XYZab"
      ((nil underline t t) (nil underline t t) (italic underline t t)
       (italic nil t t) (italic nil t t) (bold nil t t)
       (bold underline t t))))

(qa (qa241-properties '(1 3 3 8) t)
    '("--XYZab"
      ((nil underline t t) (nil underline t t) (italic underline t t)
       (italic nil t t) (italic nil t t) (bold nil t t)
       (bold underline t t))))

(qa (qa241-properties '(5 8 1 3) nil)
    '("XYZ--ab"
      ((italic underline t t) (italic nil t t) (italic nil t t)
       (nil underline t t) (nil underline t t) (bold nil t t)
       (bold underline t t))))

(qa (qa241-properties '(5 8 1 3) t)
    '("XYZ--ab"
      ((italic underline t t) (italic nil t t) (italic nil t t)
       (nil underline t t) (nil underline t t) (bold nil t t)
       (bold underline t t))))


(princ (qa--done)) (terpri)
