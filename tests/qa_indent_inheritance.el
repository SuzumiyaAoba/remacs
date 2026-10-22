;;; qa_indent_inheritance.el --- Indentation inherits sticky text properties -*- lexical-binding: t; -*-
;; Inspect inserted text and per-character faces through public commands.

(defun qa315-snapshot ()
  (list (buffer-substring-no-properties (point-min) (point-max))
        (mapcar (lambda (position) (get-text-property position 'face))
                (number-sequence (point-min) (1- (point-max))))))

(defun qa315-indent (properties position column &optional minimum tabs)
  (with-temp-buffer
    (setq indent-tabs-mode tabs tab-width 8)
    (insert (apply #'propertize "a" properties))
    (goto-char position)
    (list (indent-to column minimum) (qa315-snapshot))))

(defun qa315-public (operation text)
  (with-temp-buffer
    (setq indent-tabs-mode nil tab-width 8 fill-column 10)
    (insert (propertize text 'face 'bold 'front-sticky '(face)))
    (goto-char 1)
    (cond ((eq operation 'rigid) (indent-rigidly 1 (point-max) 2))
          ((eq operation 'center) (center-region 1 (point-max)))
          ((eq operation 'open) (open-rectangle 1 5))
          ((eq operation 'clear) (clear-rectangle 1 5))
          ((eq operation 'tabify) (tabify 1 (point-max)))
          ((eq operation 'untabify) (untabify 1 (point-max))))
    (qa315-snapshot)))

;; rear sticky default
(qa (qa315-indent '(face bold) 2 4)
    '(4 ("a   " (bold bold bold bold))))

;; front sticky face
(qa (qa315-indent '(face bold front-sticky (face)) 1 3)
    '(3 ("   a" (bold bold bold bold))))

;; front nonsticky default
(qa (qa315-indent '(face bold) 1 3)
    '(3 ("   a" (nil nil nil bold))))

;; rear nonsticky face
(qa (qa315-indent '(face bold rear-nonsticky (face)) 2 3)
    '(3 ("a  " (bold nil nil))))

;; rear nonsticky all
(qa (qa315-indent '(face bold rear-nonsticky t) 2 3)
    '(3 ("a  " (bold nil nil))))

;; front sticky all
(qa (qa315-indent '(face bold front-sticky t) 1 3)
    '(3 ("   a" (bold bold bold bold))))

;; tabs inherit
(qa (qa315-indent '(face bold) 2 16 nil t)
    '(16 ("a		" (bold bold bold))))

;; spaces inherit
(qa (qa315-indent '(face bold) 2 16 nil nil)
    '(16 ("a               " (bold bold bold bold bold bold bold bold bold bold bold bold bold bold bold bold))))

;; minimum spaces inherit
(qa (qa315-indent '(face bold) 2 0 3)
    '(4 ("a   " (bold bold bold bold))))

;; already at target
(qa (qa315-indent '(face bold) 2 1)
    '(1 ("a" (bold))))

;; rigid indentation
(qa (qa315-public 'rigid "ab\ncd\n")
    '("  ab
  cd
" (bold bold bold bold bold bold bold bold bold bold)))

;; center region
(qa (qa315-public 'center "a\nbb\n")
    '("    a
    bb
" (bold bold bold bold bold bold bold bold bold bold bold bold bold)))

;; open rectangle
(qa (qa315-public 'open "ab\ncd\n")
    '(" ab
 cd
" (bold bold bold bold bold bold bold bold)))

;; clear rectangle
(qa (qa315-public 'clear "ab\ncd\n")
    '(" b
 d
" (bold bold bold bold bold bold)))

;; tabify
(qa (qa315-public 'tabify "a       z\n")
    '("a	z
" (bold bold bold bold)))

;; untabify
(qa (qa315-public 'untabify "a\tz\n")
    '("a       z
" (bold bold bold bold bold bold bold bold bold bold)))

;; Centering reaches the separate indent-line-to primitive.
(qa (with-temp-buffer
      (setq indent-tabs-mode nil)
      (insert (propertize "a" 'face 'bold 'front-sticky '(face)))
      (list (indent-line-to 4) (qa315-snapshot)))
    '(4 ("    a" (bold bold bold bold bold))))

;; Reusing insertion must not turn an empty indentation into a read-only edit.
(qa (with-temp-buffer (setq buffer-read-only t) (indent-line-to 0)) nil)

(princ (qa--done))
