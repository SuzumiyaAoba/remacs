;;; -*- lexical-binding: t; -*-
(defun qa274-observe (text)
 (list (mapcar (lambda (p) (get-text-property p 'face text)) (number-sequence 0 (1- (length text))))
  (multibyte-string-p text) (string-to-list text)))
(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold) "b"
	      (propertize "c" 'face 'italic)))
     (replacement (propertize "X" 'face 'underline)))
  (string-match "b" source)
  (qa274-observe (replace-match replacement nil nil source)))
'((bold underline italic) nil (97 88 99))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold) "b"
	      (propertize "c" 'face 'italic)))
     (replacement (propertize "X" 'face 'underline)))
  (string-match "b" source)
  (qa274-observe (replace-match replacement t nil source)))
'((bold underline italic) nil (97 88 99))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold) "b"
	      (propertize "c" 'face 'italic)))
     (replacement (propertize "X" 'face 'underline)))
  (string-match "b" source)
  (qa274-observe (replace-match replacement nil t source)))
'((bold underline italic) nil (97 88 99))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold) "b"
	      (propertize "c" 'face 'italic)))
     (replacement (propertize "X" 'face 'underline)))
  (string-match "b" source)
  (qa274-observe (replace-match replacement t t source)))
'((bold underline italic) nil (97 88 99))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold)
	      (propertize "b" 'face 'italic)))
     (replacement (propertize "\\1" 'face 'underline)))
  (string-match "\\(a\\)\\(b\\)" source)
  (qa274-observe (replace-match replacement t nil source)))
'((bold) nil (97))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold)
	      (propertize "b" 'face 'italic)))
     (replacement (propertize "<\\1>" 'face 'underline)))
  (string-match "\\(a\\)\\(b\\)" source)
  (qa274-observe (replace-match replacement t nil source)))
'((underline bold underline) nil (60 97 62))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold)
	      (propertize "b" 'face 'italic)))
     (replacement (propertize "\\\\" 'face 'underline)))
  (string-match "\\(a\\)\\(b\\)" source)
  (qa274-observe (replace-match replacement t nil source)))
'((underline) nil (92))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold)
	      (propertize "b" 'face 'italic)))
     (replacement (propertize "\\?" 'face 'underline)))
  (string-match "\\(a\\)\\(b\\)" source)
  (qa274-observe (replace-match replacement t nil source)))
'((underline underline) nil (92 63))
)

(qa
(let*
    ((source
      (concat (propertize "a" 'face 'bold)
	      (propertize "b" 'face 'italic)))
     (replacement (propertize "\\2\\1" 'face 'underline)))
  (string-match "\\(a\\)\\(b\\)" source)
  (qa274-observe (replace-match replacement t nil source)))
'((italic bold) nil (98 97))
)

(qa
(let*
    ((source (unibyte-string 128 97 255))
     (replacement (if nil (string-as-multibyte "X") "X")))
  (string-match "a" source)
  (qa274-observe (replace-match replacement t nil source)))
'((nil nil nil) nil (128 88 255))
)

(qa
(let*
    ((source (unibyte-string 128 97 255))
     (replacement (if nil (string-as-multibyte "X") "X")))
  (string-match "a" source)
  (qa274-observe (replace-match replacement t t source)))
'((nil nil nil) nil (128 88 255))
)

(qa
(let*
    ((source (unibyte-string 128 97 255))
     (replacement (if t (string-as-multibyte "X") "X")))
  (string-match "a" source)
  (qa274-observe (replace-match replacement t nil source)))
'((nil nil nil) t (4194176 88 4194303))
)

(qa
(let*
    ((source (unibyte-string 128 97 255))
     (replacement (if t (string-as-multibyte "X") "X")))
  (string-match "a" source)
  (qa274-observe (replace-match replacement t t source)))
'((nil nil nil) t (4194176 88 4194303))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "X" 'face 'underline) t nil)
  (qa274-observe (buffer-string)))
'((bold underline warning) t (97 88 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "\\1" 'face 'underline) t nil)
  (qa274-observe (buffer-string)))
'((bold nil warning) t (97 98 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "<\\1>" 'face 'underline) t nil)
  (qa274-observe (buffer-string)))
'((bold nil nil nil warning) t (97 60 98 62 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "\\\\" 'face 'underline) t nil)
  (qa274-observe (buffer-string)))
'((bold nil warning) t (97 92 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "X" 'face 'underline) t t)
  (qa274-observe (buffer-string)))
'((bold underline warning) t (97 88 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "\\1" 'face 'underline) t t)
  (qa274-observe (buffer-string)))
'((bold underline underline warning) t (97 92 49 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "<\\1>" 'face 'underline) t t)
  (qa274-observe (buffer-string)))
'((bold underline underline underline underline warning) t
  (97 60 92 49 62 99))
)

(qa
(with-temp-buffer
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (goto-char 1) (re-search-forward "\\(b\\)")
  (replace-match (propertize "\\\\" 'face 'underline) t t)
  (qa274-observe (buffer-string)))
'((bold underline underline warning) t (97 92 92 99))
)

(qa
(let
    ((source
      (concat (propertize "a" 'face 'bold) "b"
	      (propertize "c" 'face 'italic))))
  (qa274-observe
   (replace-regexp-in-string "b" (propertize "X" 'face 'underline)
			     source t)))
'((bold underline italic) nil (97 88 99))
)

(qa
(let (events)
  (with-temp-buffer
    (insert "abc") (goto-char 1) (re-search-forward "b")
    (add-hook 'after-change-functions
	      (lambda (beg end old)
		(push (list beg end old (get-text-property beg 'face))
		      events))
	      nil t)
    (replace-match (propertize "X" 'face 'underline) t t)
    (list (nreverse events) (qa274-observe (buffer-string)))))
'(((2 3 1 underline)) ((nil underline nil) t (97 88 99)))
)

(qa
(with-temp-buffer
  (buffer-enable-undo)
  (insert
   (concat (propertize "a" 'face 'bold) (propertize "b" 'face 'italic)
	   (propertize "c" 'face 'warning)))
  (setq buffer-undo-list nil) (goto-char 1) (re-search-forward "b")
  (replace-match (propertize "XYZ" 'face 'underline) t t)
  (primitive-undo 1 buffer-undo-list) (qa274-observe (buffer-string)))
'((bold italic warning) t (97 98 99))
)

(qa
(let ((source (propertize "a" 'face 'bold)))
  (string-match "\\(a\\)\\(b\\)?" source)
  (qa274-observe (replace-match "\\2\\1" t nil source)))
'((bold) nil (97))
)

(qa
(let
    ((source "a")
     (replacement
      (concat (propertize "\\" 'face 'bold)
	      (propertize "\\" 'face 'italic))))
  (string-match "a" source)
  (qa274-observe (replace-match replacement t nil source)))
'((bold) nil (92))
)

(qa
(let ((source "AB"))
  (string-match "AB" source)
  (qa274-observe
   (replace-match (propertize "x" 'face 'underline) nil t source)))
'((underline) nil (88))
)

(qa
(with-temp-buffer
  (insert "AB") (goto-char 1) (re-search-forward "AB")
  (replace-match (propertize "x" 'face 'underline) nil t)
  (qa274-observe (buffer-string)))
'((underline) t (88))
)

(qa
(let ((source "AB"))
  (string-match "AB" source)
  (qa274-observe
   (replace-match (propertize "\x00df" 'face 'underline) nil t source)))
'((nil nil) t (83 83))
)

(qa
(with-temp-buffer
  (insert "AB") (goto-char 1) (re-search-forward "AB")
  (replace-match (propertize "\x00df" 'face 'underline) nil t)
  (qa274-observe (buffer-string)))
'((nil underline) t (83 83))
)

(qa
(let ((source "AB"))
  (string-match "AB" source)
  (qa274-observe
   (replace-match
    (propertize (string-as-multibyte "x") 'face 'underline) nil t
    source)))
'((nil) t (88))
)

(qa
(with-temp-buffer
  (insert "AB") (goto-char 1) (re-search-forward "AB")
  (replace-match
   (propertize (string-as-multibyte "x") 'face 'underline) nil t)
  (qa274-observe (buffer-string)))
'((underline) t (88))
)


(princ (qa--done)) (terpri)
