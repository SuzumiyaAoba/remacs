;;; -*- lexical-binding: t; -*-
;; Issue #57: table lookup must use the original Lisp character code,
;; including surrogate, high Mule, and eight-bit proxy characters.
(defvar qa57-code)
(defvar qa57-string)
(dolist (qa57-code '(#x80 #xE9 #x65E5 #x301 #x200D #xD800
                    #x110000 #x140000 #x1416EF #x3FFF80 #x3FFFFF))
  (let ((char-width-table (copy-sequence char-width-table))
        (qa57-string (string qa57-code)))
    (set-char-table-range char-width-table qa57-code 5)
    (qa (char-width qa57-code) 5)
    (qa (string-width (concat qa57-string qa57-string)) 10)
    (qa (truncate-string-to-width (concat qa57-string qa57-string) 6) qa57-string)
    (set-char-table-range char-width-table qa57-code 0)
    (qa (char-width qa57-code) 0)
    (qa (string-width (concat qa57-string qa57-string)) 0)
    (qa (truncate-string-to-width (concat qa57-string "a") 0) "")))

(qa (let ((char-width-table (copy-sequence char-width-table)))
      (set-char-table-range char-width-table ?日 5)
      (list (char-width ?日) (string-width "日日")
            (truncate-string-to-width "日日" 6))) '(5 10 "日"))
(qa (let ((char-width-table (copy-sequence char-width-table)))
      (set-char-table-range char-width-table ?日 3)
      (list (string-width "x日日y" 1 -1)
            (truncate-string-to-width "日日日" 7 3)
            (truncate-string-to-width "日日日" 7 0 nil "日"))) '(6 "日" "日日"))

;; Nil child entries inherit from the parent; mutation is visible on
;; the next call. The original width table survives dynamic rebinding.
(defvar qa57-parent)
(qa (let* ((qa57-parent (copy-sequence char-width-table))
           (char-width-table (make-char-table nil nil)))
      (set-char-table-range qa57-parent ?日 4)
      (set-char-table-parent char-width-table qa57-parent)
      (let ((first (char-width ?日)))
        (set-char-table-range qa57-parent ?日 6)
        (list first (char-width ?日) (string-width "日")))) '(4 6 6))
(qa (char-width ?日) 2)
(qa (let ((char-width-table (copy-sequence char-width-table)))
      (set-char-table-range char-width-table ?日 0)
      (list (truncate-string-to-width "日a" 1)
            (truncate-string-to-width "a日" 1)
            (truncate-string-to-width "a日" 1 0 nil "..."))) '("日a" "a" "a"))
(qa (with-temp-buffer
      (set (make-local-variable 'char-width-table) (copy-sequence char-width-table))
      (set-char-table-range char-width-table ?日 7)
      (list (char-width ?日) (string-width "日"))) '(7 7))
(qa (char-width ?日) 2)

;; CHARACTER_WIDTH handles ASCII directly, independently of the table.
(qa (let ((char-width-table (copy-sequence char-width-table)))
      (dolist (c '(?A 1 10 127)) (set-char-table-range char-width-table c 5))
      (list (char-width ?A) (string-width "AA") (char-width 1)
            (char-width 10) (char-width 127))) '(1 2 2 0 2))

(princ (qa--done)) (terpri)
