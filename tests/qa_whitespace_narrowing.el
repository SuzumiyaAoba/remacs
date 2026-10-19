;;; -*- lexical-binding: nil -*-
;; Issue #270: whitespace commands only edit the accessible buffer.
;; Expectations verified against GNU Emacs 31.1.  Source properties and
;; markers expose which characters were removed, even for equal text.
;; Capture bounds, point, text, per-character identity, and markers on each side.
(defun qa270-snapshot (text lo hi position command)
  (with-temp-buffer
    (insert text)
    (let ((pos 1) (limit (point-max)))
      (while (< pos limit)
        (put-text-property pos (1+ pos) 'qa270-source pos)
        (setq pos (1+ pos)))
      (let ((markers (mapcar #'copy-marker (list 1 lo position hi limit)))
            (after (copy-marker position t)))
        (narrow-to-region lo hi)
        (goto-char position)
        (let* ((result (eval command))
               (bounds (list (point-min) (point-max) (point)))
               (visible (buffer-substring-no-properties (point-min) (point-max))))
          (widen)
          (let ((pos 1) properties)
            (while (< pos (point-max))
              (push (get-text-property pos 'qa270-source) properties)
              (setq pos (1+ pos)))
            (list result bounds visible
                  (buffer-substring-no-properties (point-min) (point-max))
                  (nreverse properties) (mapcar #'marker-position markers)
                  (marker-position after))))))))

;; reported blank line
(qa (qa270-snapshot "head\n\n\nbody\n" 6 7 6 '(delete-blank-lines))
    '(nil (6 6 6) "" "head\n\nbody\n" (1 2 3 4 5 7 8 9 10 11 12) (1 6 6 6 12) 6))

;; reported horizontal space
(qa (qa270-snapshot "A    B" 3 5 4 '(delete-horizontal-space))
    '(nil (3 3 3) "" "A  B" (1 2 5 6) (1 3 3 3 5) 3))

;; reported indentation
(qa (qa270-snapshot "head\n  body" 8 12 8 '(delete-indentation))
    '(nil (8 12 8) "body" "head\n  body" (1 2 3 4 5 6 7 8 9 10 11) (1 8 8 12 12) 8))

;; just-one-space control
(qa (qa270-snapshot "A    B" 3 5 4 '(just-one-space))
    '(nil (3 4 4) " " "A   B" (1 2 3 5 6) (1 3 4 4 6) 4))

;; horizontal edges nil3
(qa (qa270-snapshot "界 \t  文" 3 5 3 '(delete-horizontal-space nil))
    '(nil (3 3 3) "" "界  文" (1 2 5 6) (1 3 3 3 5) 3))

;; horizontal edges nil4
(qa (qa270-snapshot "界 \t  文" 3 5 4 '(delete-horizontal-space nil))
    '(nil (3 3 3) "" "界  文" (1 2 5 6) (1 3 3 3 5) 3))

;; horizontal edges nil5
(qa (qa270-snapshot "界 \t  文" 3 5 5 '(delete-horizontal-space nil))
    '(nil (3 3 3) "" "界  文" (1 2 5 6) (1 3 3 3 5) 3))

;; horizontal edges t3
(qa (qa270-snapshot "界 \t  文" 3 5 3 '(delete-horizontal-space t))
    '(nil (3 5 3) "	 " "界 	  文" (1 2 3 4 5 6) (1 3 3 5 7) 3))

;; horizontal edges t4
(qa (qa270-snapshot "界 \t  文" 3 5 4 '(delete-horizontal-space t))
    '(nil (3 4 3) " " "界   文" (1 2 4 5 6) (1 3 3 4 6) 3))

;; horizontal edges t5
(qa (qa270-snapshot "界 \t  文" 3 5 5 '(delete-horizontal-space t))
    '(nil (3 3 3) "" "界  文" (1 2 5 6) (1 3 3 3 5) 3))

;; empty restriction (delete-blank-lines)
(qa (qa270-snapshot "x \n y" 3 3 3 '(delete-blank-lines))
    '(nil (3 3 3) "" "x \n y" (1 2 3 4 5) (1 3 3 3 6) 3))

;; empty buffer (delete-blank-lines)
(qa (qa270-snapshot "" 1 1 1 '(delete-blank-lines))
    '(nil (1 1 1) "" "" nil (1 1 1 1 1) 1))

;; empty restriction (delete-horizontal-space)
(qa (qa270-snapshot "x \n y" 3 3 3 '(delete-horizontal-space))
    '(nil (3 3 3) "" "x \n y" (1 2 3 4 5) (1 3 3 3 6) 3))

;; empty buffer (delete-horizontal-space)
(qa (qa270-snapshot "" 1 1 1 '(delete-horizontal-space))
    '(nil (1 1 1) "" "" nil (1 1 1 1 1) 1))

;; empty restriction (delete-indentation)
(qa (qa270-snapshot "x \n y" 3 3 3 '(delete-indentation))
    '(nil (3 3 3) "" "x \n y" (1 2 3 4 5) (1 3 3 3 6) 3))

;; empty buffer (delete-indentation)
(qa (qa270-snapshot "" 1 1 1 '(delete-indentation))
    '(nil (1 1 1) "" "" nil (1 1 1 1 1) 1))

;; empty restriction (delete-indentation t)
(qa (qa270-snapshot "x \n y" 3 3 3 '(delete-indentation t))
    '(nil (3 3 3) "" "x \n y" (1 2 3 4 5) (1 3 3 3 6) 3))

;; empty buffer (delete-indentation t)
(qa (qa270-snapshot "" 1 1 1 '(delete-indentation t))
    '(nil (1 1 1) "" "" nil (1 1 1 1 1) 1))

;; empty restriction (just-one-space 0)
(qa (qa270-snapshot "x \n y" 3 3 3 '(just-one-space 0))
    '(nil (3 3 3) "" "x \n y" (1 2 3 4 5) (1 3 3 3 6) 3))

;; empty buffer (just-one-space 0)
(qa (qa270-snapshot "" 1 1 1 '(just-one-space 0))
    '(nil (1 1 1) "" "" nil (1 1 1 1 1) 1))

;; blank line boundaries
(qa (qa270-snapshot "head\n\n\nbody\n" 6 8 7 '(delete-blank-lines))
    '(nil (6 6 6) "" "head\nbody\n" (1 2 3 4 5 8 9 10 11 12) (1 6 6 6 11) 6))

;; blank line boundaries
(qa (qa270-snapshot "head\n \t\n \nbody" 7 10 8 '(delete-blank-lines))
    '(nil (7 7 7) "" "head\n \nbody" (1 2 3 4 5 6 10 11 12 13 14) (1 7 7 7 12) 7))

;; blank line boundaries
(qa (qa270-snapshot "head \n \n \nbody" 6 10 8 '(delete-blank-lines))
    '(nil (6 6 6) "" "head \nbody" (1 2 3 4 5 10 11 12 13 14) (1 6 6 6 11) 6))

;; blank line boundaries
(qa (qa270-snapshot "x\n\n\ny" 2 4 2 '(delete-blank-lines))
    '(nil (2 2 2) "" "x\ny" (1 4 5) (1 2 2 2 4) 2))

;; blank line boundaries
(qa (qa270-snapshot "x\n\n\ny" 3 5 3 '(delete-blank-lines))
    '(nil (3 3 3) "" "x\ny" (1 2 5) (1 3 3 3 4) 3))

;; blank line boundaries
(qa (qa270-snapshot " \n \n " 1 6 3 '(delete-blank-lines))
    '(nil (1 1 1) "" "" nil (1 1 1 1 1) 1))

;; blank line boundaries
(qa (qa270-snapshot "head\n\n" 6 7 6 '(delete-blank-lines))
    '(nil (6 6 6) "" "head\n" (1 2 3 4 5) (1 6 6 6 6) 6))

;; blank line boundaries
(qa (qa270-snapshot "head\n\n" 1 7 6 '(delete-blank-lines))
    '(nil (1 6 6) "head\n" "head\n" (1 2 3 4 5) (1 1 6 6 6) 6))

;; blank line boundaries
(qa (qa270-snapshot "head\n\n" 1 7 2 '(delete-blank-lines))
    '(nil (1 6 2) "head\n" "head\n" (1 2 3 4 5) (1 1 2 6 6) 2))

;; blank line boundaries
(qa (qa270-snapshot "head\n \t" 6 8 7 '(delete-blank-lines))
    '(nil (6 6 6) "" "head\n" (1 2 3 4 5) (1 6 6 6 6) 6))

;; blank line boundaries
(qa (qa270-snapshot "head\n \tTAIL" 6 8 7 '(delete-blank-lines))
    '(nil (6 6 6) "" "head\nTAIL" (1 2 3 4 5 8 9 10 11) (1 6 6 6 10) 6))

;; blank line boundaries
(qa (qa270-snapshot "A\n\n\nB" 1 6 5 '(delete-blank-lines))
    '(nil (1 6 5) "A\n\n\nB" "A\n\n\nB" (1 2 3 4 5) (1 1 5 6 6) 5))

;; blank line boundaries
(qa (qa270-snapshot "A\n\n\nB" 1 6 1 '(delete-blank-lines))
    '(nil (1 4 1) "A\nB" "A\nB" (1 2 5) (1 1 1 4 4) 1))

;; join boundaries
(qa (qa270-snapshot "A \n \tB tail" 2 7 6 '(delete-indentation nil))
    '(nil (2 3 2) "B" "AB tail" (1 6 7 8 9 10 11) (1 2 2 3 8) 2))

;; join boundaries
(qa (qa270-snapshot "A \n \tB tail" 2 7 2 '(delete-indentation t))
    '(nil (2 3 2) "B" "AB tail" (1 6 7 8 9 10 11) (1 2 2 3 8) 2))

;; join boundaries
(qa (qa270-snapshot "head\n  body" 5 12 8 '(delete-indentation nil))
    '(nil (5 9 5) "body" "headbody" (1 2 3 4 8 9 10 11) (1 5 5 9 9) 5))

;; join boundaries
(qa (qa270-snapshot "head\n  body" 6 12 10 '(delete-indentation nil))
    '(nil (6 12 6) "  body" "head\n  body" (1 2 3 4 5 6 7 8 9 10 11) (1 6 10 12 12) 10))

;; join boundaries
(qa (qa270-snapshot "head\n  body" 1 5 1 '(delete-indentation t))
    '(nil (1 5 5) "head" "head\n  body" (1 2 3 4 5 6 7 8 9 10 11) (1 1 1 5 12) 1))

;; join boundaries
(qa (qa270-snapshot "head\n  body" 1 6 1 '(delete-indentation t))
    '(nil (1 5 5) "head" "head  body" (1 2 3 4 6 7 8 9 10 11) (1 1 1 5 11) 1))

;; join boundaries
(qa (qa270-snapshot "head\n  body" 1 8 1 '(delete-indentation t))
    '(nil (1 5 5) "head" "headbody" (1 2 3 4 8 9 10 11) (1 1 1 5 9) 1))

;; join boundaries
(qa (qa270-snapshot "A\n  B\n C" 1 9 6 '(delete-indentation nil))
    '(nil (1 7 2) "A B\n C" "A B\n C" (1 nil 5 6 7 8) (1 1 4 7 7) 4))

;; join boundaries
(qa (qa270-snapshot "A\n  B\n C" 1 9 5 '(delete-indentation t))
    '(nil (1 8 6) "A\n  B C" "A\n  B C" (1 2 3 4 5 nil 8) (1 1 5 8 8) 5))

;; join boundaries
(qa (qa270-snapshot "(\n  B" 1 6 4 '(delete-indentation nil))
    '(nil (1 3 2) "(B" "(B" (1 5) (1 1 2 3 3) 2))

;; join boundaries
(qa (qa270-snapshot "A\n  )" 1 6 4 '(delete-indentation nil))
    '(nil (1 3 2) "A)" "A)" (1 5) (1 1 2 3 3) 2))

;; join boundaries
(qa (qa270-snapshot "A\n  B" 3 5 3 '(delete-indentation t))
    '(nil (3 5 5) "  " "A\n  B" (1 2 3 4 5) (1 3 3 5 6) 3))

;; join boundaries
(qa (qa270-snapshot "A\n  B" 3 5 5 '(delete-indentation nil))
    '(nil (3 5 3) "  " "A\n  B" (1 2 3 4 5) (1 3 5 5 6) 5))


;; Hidden read-only text and its properties must remain untouched.
(dolist (case '(("前 \t  後" 3 5 4 (delete-horizontal-space))
                ("前\n\n\n後" 3 4 3 (delete-blank-lines))
                ("前 A\n  B 後" 3 8 7 (delete-indentation))))
  (qa (with-temp-buffer
        (insert (nth 0 case))
        (let* ((lo (nth 1 case)) (hi (nth 2 case))
               (whole (point-max)))
          (put-text-property 1 lo 'face 'bold)
          (put-text-property hi whole 'face 'italic)
          (put-text-property 1 lo 'read-only 'hidden)
          (put-text-property hi whole 'read-only 'hidden)
          (let ((prefix (buffer-substring 1 lo))
                (suffix (buffer-substring hi whole)))
            (narrow-to-region lo hi) (goto-char (nth 3 case))
            (eval (nth 4 case))
            (let ((beg (point-min)) (end (point-max)))
              (widen)
              (list (equal-including-properties prefix (buffer-substring 1 beg))
                    (equal-including-properties suffix (buffer-substring end (point-max))))))))
      '(t t)))
;; Restricting to a partial line must not borrow a hidden newline.
(qa (with-temp-buffer
      (insert "head\n  body") (narrow-to-region 8 12)
      (goto-char 10)
      (let ((deactivate-mark nil))
        (delete-indentation)
        (list (point) (point-min) (point-max) deactivate-mark)))
    '(8 8 12 t))
;; Joining an accessible region still supports BEG/END and fill prefixes.
(qa (with-temp-buffer
      (insert "hidden\nA\n> B\n> C\nhidden")
      (narrow-to-region 8 17) (goto-char 12)
      (let ((fill-prefix "> ")) (delete-indentation nil 8 16))
      (list (point) (buffer-string))) '(9 "A B C"))
;; The following-line argument deliberately ignores region arguments.
(qa (with-temp-buffer
      (insert "A\n B\n C") (goto-char 1)
      (delete-indentation t 1 8)
      (buffer-string)) "A B\n C")
;; join-line is the same command and honors its full calling convention.
(qa (with-temp-buffer
      (insert "A\n B\n C")
      (join-line nil 1 8)
      (buffer-string)) "A B C")
;; Changes use normal hooks and undo records, including an inserted join space.
(qa (with-temp-buffer
      (insert "前 A\n  B 後")
      (buffer-enable-undo) (setq buffer-undo-list nil)
      (narrow-to-region 3 8) (goto-char 7)
      (let (events
            (after-change-functions
             (list (lambda (beg end old) (push (list beg end old) events)))))
        (delete-indentation)
        (let ((all-visible t))
          (dolist (event events)
            (unless (and (>= (car event) (point-min))
                         (<= (cadr event) (point-max)))
              (setq all-visible nil)))
          (let ((changed (buffer-string)))
            (primitive-undo 1 buffer-undo-list)
            (widen)
            (list changed (buffer-string) all-visible (> (length events) 0))))))
    '("A B" "前 A\n  B 後" t t))
;; Horizontal deletion must stop at field boundaries as well as narrowing.
(qa (with-temp-buffer
      (insert "A    B")
      (put-text-property 2 4 'field 'left)
      (put-text-property 4 6 'field 'right)
      (goto-char 5) (delete-horizontal-space)
      (list (buffer-string) (point))) '("A  B" 4))

;; Hidden raw bytes stay byte-identical in unibyte buffers too.
(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (insert (unibyte-string 255 32 32 32 32 254))
      (narrow-to-region 3 5) (goto-char 4) (delete-horizontal-space)
      (widen)
      (list (string-to-list (buffer-string)) (multibyte-string-p (buffer-string))))
    '((255 32 32 254) nil))
(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (insert (unibyte-string 255 10 10 10 254))
      (narrow-to-region 3 4) (delete-blank-lines) (widen)
      (string-to-list (buffer-string))) '(255 10 10 254))

(princ (qa--done)) (terpri)
