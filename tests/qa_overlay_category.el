;;; qa_overlay_category.el --- Effective inherited overlay properties -*- lexical-binding: t; -*-
;; Public overlay and character-property readers use the same defaults.

(defun qa312-property (property value &optional direct override text lower)
  (with-temp-buffer
    (insert "abc")
    (let ((o (make-overlay 1 4)) (category (make-symbol "qa312-category")))
      (put category property value) (overlay-put o 'category category)
      (overlay-put o 'priority 10)
      (when direct (overlay-put o property override))
      (when text (put-text-property 1 4 property text))
      (when lower
        (let ((other (make-overlay 1 4)))
          (overlay-put other property lower) (overlay-put other 'priority 1)))
      (let ((pair (get-char-property-and-overlay 2 property)))
        (list (overlay-get o property) (get-char-property 2 property)
              (get-pos-property 2 property) (car pair) (eq (cdr pair) o))))))

;; face
(qa (qa312-property 'face 'bold)
    '(bold bold bold bold t))

;; mouse-face
(qa (qa312-property 'mouse-face 'highlight)
    '(highlight highlight highlight highlight t))

;; help-echo
(qa (qa312-property 'help-echo "help")
    '("help" "help" "help" "help" t))

;; display
(qa (qa312-property 'display "replacement")
    '("replacement" "replacement" "replacement" "replacement" t))

;; read-only
(qa (qa312-property 'read-only t)
    '(t t t t t))

;; zero is a value
(qa (qa312-property 'qa312 0)
    '(0 0 0 0 t))

;; nil inheritance falls to text
(qa (qa312-property 'face nil nil nil 'underline)
    '(nil underline underline underline nil))

;; direct value wins
(qa (qa312-property 'face 'bold t 'italic)
    '(italic italic italic italic t))

;; nil shadows default
(qa (qa312-property 'face 'bold t nil)
    '(nil nil nil nil nil))

;; nil shadow falls to text
(qa (qa312-property 'face 'bold t nil 'underline)
    '(nil underline underline underline nil))

;; nil shadow falls to lower overlay
(qa (qa312-property 'face 'bold t nil nil 'italic)
    '(nil italic italic italic nil))

;; inherited high priority wins
(qa (qa312-property 'face 'bold nil nil 'underline 'italic)
    '(bold bold bold bold t))

;; dynamic category defaults
(qa (with-temp-buffer (insert "abc") (let ((o (make-overlay 1 4)) (cat (make-symbol "qa312-dynamic"))) (overlay-put o 'category cat) (put cat 'face 'bold) (let ((before (get-char-property 2 'face))) (put cat 'face 'italic) (list before (get-char-property 2 'face)))))
    '(bold italic))

;; category replacement
(qa (with-temp-buffer (insert "abc") (let ((o (make-overlay 1 4)) (a (make-symbol "qa312-a")) (b (make-symbol "qa312-b"))) (put a 'face 'bold) (put b 'face 'italic) (overlay-put o 'category a) (let ((before (get-char-property 2 'face))) (overlay-put o 'category b) (list before (get-char-property 2 'face)))))
    '(bold italic))

;; string objects have no overlays
(qa (with-temp-buffer (insert "abc") (let ((o (make-overlay 1 4)) (cat (make-symbol "qa312-string"))) (put cat 'face 'bold) (overlay-put o 'category cat) (get-char-property 1 'face (propertize "abc" 'face 'underline))))
    'underline)

(princ (qa--done))
