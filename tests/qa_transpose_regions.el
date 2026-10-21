;;; -*- lexical-binding: t; -*-
;; Issue #240: native transposition relocates point/markers, preserves
;; overlays and properties, and emits one change with GNU undo records.
(defun qa240-positions (text regions leave)
  (let ((positions (number-sequence 1 (1+ (length text)))))
    (list
     (mapcar (lambda (position) (with-temp-buffer (insert text) (goto-char position)
               (apply #'transpose-regions (append regions (list leave))) (point))) positions)
     (mapcar (lambda (mode) (with-temp-buffer (insert text)
               (let ((markers (mapcar (lambda (position) (copy-marker position mode)) positions)))
                 (apply #'transpose-regions (append regions (list leave)))
                 (list (buffer-string) (mapcar #'marker-position markers))))) '(nil t)))))
(defvar qa240-events nil)
(defun qa240-change (regions style)
  (with-temp-buffer (insert "ab--XYZ") (goto-char 2)
    (when style (put-text-property 1 3 'face 'bold)
      (put-text-property 3 5 'face 'underline) (put-text-property 5 8 'face 'italic))
    (buffer-enable-undo) (setq buffer-undo-list nil) (set-buffer-modified-p nil)
    (let ((tick (buffer-modified-tick)) (chars (buffer-chars-modified-tick)) (qa240-events nil)
          (before-change-functions (list (lambda (&rest args) (push (cons 'before args) qa240-events))))
          (after-change-functions (list (lambda (&rest args) (push (cons 'after args) qa240-events)))))
      (apply #'transpose-regions regions)
      (list (buffer-substring-no-properties 1 8) (point)
            (- (buffer-modified-tick) tick) (- (buffer-chars-modified-tick) chars)
            (reverse qa240-events) buffer-undo-list (object-intervals (buffer-string))))))

(qa (qa240-positions "ab--XYZ" '(1 3 5 8) nil)
    '((6 7 4 5 1 2 3 8)
      (("XYZ--ab" (6 7 4 5 1 2 3 8)) ("XYZ--ab" (6 7 4 5 1 2 3 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 5 8) t)
    '((1 2 3 4 5 6 7 8)
      (("XYZ--ab" (1 2 3 4 5 6 7 8)) ("XYZ--ab" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 5 7) nil)
    '((5 6 3 4 1 2 7 8)
      (("XY--abZ" (5 6 3 4 1 2 7 8)) ("XY--abZ" (5 6 3 4 1 2 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 5 7) t)
    '((1 2 3 4 5 6 7 8)
      (("XY--abZ" (1 2 3 4 5 6 7 8)) ("XY--abZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 3 5) nil)
    '((3 4 1 2 5 6 7 8)
      (("--abXYZ" (3 4 1 2 5 6 7 8)) ("--abXYZ" (3 4 1 2 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 3 5) t)
    '((1 2 3 4 5 6 7 8)
      (("--abXYZ" (1 2 3 4 5 6 7 8)) ("--abXYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 1 5 8) nil)
    '((4 5 6 7 1 2 3 8)
      (("XYZab--" (4 5 6 7 1 2 3 8)) ("XYZab--" (4 5 6 7 1 2 3 8)))))

(qa (qa240-positions "ab--XYZ" '(1 1 5 8) t)
    '((1 2 3 4 5 6 7 8)
      (("XYZab--" (1 2 3 4 5 6 7 8)) ("XYZab--" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 5 5) nil)
    '((3 4 1 2 5 6 7 8)
      (("--abXYZ" (3 4 1 2 5 6 7 8)) ("--abXYZ" (3 4 1 2 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 3 5 5) t)
    '((1 2 3 4 5 6 7 8)
      (("--abXYZ" (1 2 3 4 5 6 7 8)) ("--abXYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 1 5 5) nil)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 1 5 5) t)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 1 1 3) nil)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 1 1 3) t)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(3 1 8 5) nil)
    '((6 7 4 5 1 2 3 8)
      (("XYZ--ab" (6 7 4 5 1 2 3 8)) ("XYZ--ab" (6 7 4 5 1 2 3 8)))))

(qa (qa240-positions "ab--XYZ" '(3 1 8 5) t)
    '((1 2 3 4 5 6 7 8)
      (("XYZ--ab" (1 2 3 4 5 6 7 8)) ("XYZ--ab" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(5 8 1 3) nil)
    '((6 7 4 5 1 2 3 8)
      (("XYZ--ab" (6 7 4 5 1 2 3 8)) ("XYZ--ab" (6 7 4 5 1 2 3 8)))))

(qa (qa240-positions "ab--XYZ" '(5 8 1 3) t)
    '((1 2 3 4 5 6 7 8)
      (("XYZ--ab" (1 2 3 4 5 6 7 8)) ("XYZ--ab" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(3 3 3 3) nil)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(3 3 3 3) t)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 8 8 8) nil)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(1 8 8 8) t)
    '((1 2 3 4 5 6 7 8)
      (("ab--XYZ" (1 2 3 4 5 6 7 8)) ("ab--XYZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "ab--XYZ" '(2 3 4 7) nil)
    '((1 6 5 2 3 4 7 8)
      (("a-XY-bZ" (1 6 5 2 3 4 7 8)) ("a-XY-bZ" (1 6 5 2 3 4 7 8)))))

(qa (qa240-positions "ab--XYZ" '(2 3 4 7) t)
    '((1 2 3 4 5 6 7 8)
      (("a-XY-bZ" (1 2 3 4 5 6 7 8)) ("a-XY-bZ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-positions "αβ--日本語" '(1 3 5 8) nil)
    '((6 7 4 5 1 2 3 8)
      (("日本語--αβ" (6 7 4 5 1 2 3 8))
       ("日本語--αβ" (6 7 4 5 1 2 3 8)))))

(qa (qa240-positions "αβ--日本語" '(1 3 5 8) t)
    '((1 2 3 4 5 6 7 8)
      (("日本語--αβ" (1 2 3 4 5 6 7 8))
       ("日本語--αβ" (1 2 3 4 5 6 7 8)))))

(qa (qa240-change '(1 3 5 8) nil)
    '("XYZ--ab" 7 3 3 ((before 1 8) (after 1 8 7))
      ((1 . 8) ("ab--XYZ" . 1) (t . 0)) nil))

(qa (qa240-change '(1 3 5 8) t)
    '("XYZ--ab" 7 3 6 ((before 1 8) (after 1 8 7))
      ((nil face italic 5 . 8) (nil face underline 3 . 5)
       (nil face bold 1 . 3) (1 . 8)
       (#("ab--XYZ" 0 2 (face bold) 2 4 (face underline) 4 7
	  (face italic))
	. 1)
       (t . 0))
      ((0 3 (face italic)) (3 5 (face underline)) (5 7 (face bold)))))

(qa (qa240-change '(1 3 5 7) nil)
    '("XY--abZ" 6 3 3 ((before 1 7) (after 1 7 6))
      ((5 . 7) ("XY" . 5) (1 . 3) ("ab" . 1) (t . 0)) nil))

(qa (qa240-change '(1 3 5 7) t)
    '("XY--abZ" 6 3 6 ((before 1 7) (after 1 7 6))
      ((nil face bold 1 . 3) (5 . 7) (#("XY" 0 2 (face italic)) . 5)
       (1 . 3) (#("ab" 0 2 (face bold)) . 1) (t . 0))
      ((0 2 (face italic)) (2 4 (face underline)) (4 6 (face bold))
       (6 7 (face italic)))))

(qa (qa240-change '(1 3 3 5) nil)
    '("--abXYZ" 4 3 3 ((before 1 5) (after 1 5 4))
      ((1 . 5) ("ab--" . 1) (t . 0)) nil))

(qa (qa240-change '(1 3 3 5) t)
    '("--abXYZ" 4 3 6 ((before 1 5) (after 1 5 4))
      ((nil face underline 3 . 5) (nil face bold 1 . 3) (1 . 5)
       (#("ab--" 0 2 (face bold) 2 4 (face underline)) . 1) (t . 0))
      ((0 2 (face underline)) (2 4 (face bold)) (4 7 (face italic)))))

(qa (qa240-change '(1 1 5 8) nil)
    '("XYZab--" 5 3 3 ((before 1 8) (after 1 8 7))
      ((1 . 8) ("ab--XYZ" . 1) (t . 0)) nil))

(qa (qa240-change '(1 1 5 8) t)
    '("XYZab--" 5 3 6 ((before 1 8) (after 1 8 7))
      ((nil face italic 5 . 8) (nil face underline 3 . 5)
       (nil face bold 1 . 3) (1 . 8)
       (#("ab--XYZ" 0 2 (face bold) 2 4 (face underline) 4 7
	  (face italic))
	. 1)
       (t . 0))
      ((0 3 (face italic)) (3 5 (face bold)) (5 7 (face underline)))))

(qa (qa240-change '(1 1 5 5) nil)
    '("ab--XYZ" 2 3 3 ((before 1 5) (after 1 5 4))
      ((5 . 5) ("" . 5) (1 . 1) ("" . 1) (t . 0)) nil))

(qa (qa240-change '(1 1 5 5) t)
    '("ab--XYZ" 2 3 6 ((before 1 5) (after 1 5 4))
      ((5 . 5) ("" . 5) (1 . 1) ("" . 1) (t . 0))
      ((0 2 (face bold)) (2 4 (face underline)) (4 7 (face italic)))))

(qa (qa240-change '(1 1 1 3) nil) '("ab--XYZ" 2 0 0 nil nil nil))

(qa (qa240-change '(1 1 1 3) t)
    '("ab--XYZ" 2 0 0 nil nil
      ((0 2 (face bold)) (2 4 (face underline)) (4 7 (face italic)))))

(qa (qa240-change '(5 8 1 3) nil)
    '("XYZ--ab" 7 3 3 ((before 1 8) (after 1 8 7))
      ((1 . 8) ("ab--XYZ" . 1) (t . 0)) nil))

(qa (qa240-change '(5 8 1 3) t)
    '("XYZ--ab" 7 3 6 ((before 1 8) (after 1 8 7))
      ((nil face underline 3 . 5) (1 . 8)
       (#("ab--XYZ" 0 2 (face bold) 2 4 (face underline) 4 7
	  (face italic))
	. 1)
       (t . 0))
      ((0 2 (face italic)) (2 3 (face italic)) (3 4 (face underline))
       (4 5 (face underline)) (5 7 (face bold)))))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((a (make-overlay 1 3)) (b (make-overlay 3 5))
	(c (make-overlay 5 8)))
     (transpose-regions 1 3 5 8 nil)
     (mapcar
      (lambda (overlay)
	(list (overlay-start overlay) (overlay-end overlay)))
      (list a b c))))
 '((1 3) (3 5) (5 8)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 2)
   (let*
       ((base (current-buffer)) (marker (copy-marker 2))
	(view (make-indirect-buffer base " *qa240-view*")))
     (unwind-protect
	 (with-current-buffer view
	   (narrow-to-region 2 7) (goto-char 6)
	   (let ((m (copy-marker 6)))
	     (transpose-regions 2 3 5 7 nil)
	     (list (buffer-string) (point) (point-min) (point-max)
		   (marker-position m)
		   (with-current-buffer base
		     (list (buffer-string) (point)
			   (marker-position marker))))))
       (kill-buffer view))))
 '("XY--b" 3 2 7 3 ("aXY--bZ" 6 6)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let*
       ((base (current-buffer))
	(view (make-indirect-buffer base " *qa240-view*")))
     (unwind-protect
	 (progn
	   (with-current-buffer view
	     (narrow-to-region 1 2) (goto-char 1))
	   (transpose-regions 1 3 5 8 nil)
	   (with-current-buffer view
	     (list (point-min) (point-max) (point) (buffer-string))))
       (kill-buffer view))))
 '(6 7 6 "a"))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((a (make-overlay 1 3)) (b (make-overlay 3 5))
	(c (make-overlay 5 8)))
     (transpose-regions 1 3 5 8 t)
     (mapcar
      (lambda (overlay)
	(list (overlay-start overlay) (overlay-end overlay)))
      (list a b c))))
 '((1 3) (3 5) (5 8)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 2)
   (let*
       ((base (current-buffer)) (marker (copy-marker 2))
	(view (make-indirect-buffer base " *qa240-view*")))
     (unwind-protect
	 (with-current-buffer view
	   (narrow-to-region 2 7) (goto-char 6)
	   (let ((m (copy-marker 6)))
	     (transpose-regions 2 3 5 7 t)
	     (list (buffer-string) (point) (point-min) (point-max)
		   (marker-position m)
		   (with-current-buffer base
		     (list (buffer-string) (point)
			   (marker-position marker))))))
       (kill-buffer view))))
 '("XY--b" 6 2 7 6 ("aXY--bZ" 2 2)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let*
       ((base (current-buffer))
	(view (make-indirect-buffer base " *qa240-view*")))
     (unwind-protect
	 (progn
	   (with-current-buffer view
	     (narrow-to-region 1 2) (goto-char 1))
	   (transpose-regions 1 3 5 8 t)
	   (with-current-buffer view
	     (list (point-min) (point-max) (point) (buffer-string))))
       (kill-buffer view))))
 '(1 2 1 "X"))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((buffer-read-only t))
     (condition-case err (apply #'transpose-regions '(1 1 1 1))
       (error (car err)))))
 'nil)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((buffer-read-only t))
     (condition-case err (apply #'transpose-regions '(1 1 1 3))
       (error (car err)))))
 'nil)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((buffer-read-only t))
     (condition-case err (apply #'transpose-regions '(1 1 3 3))
       (error (car err)))))
 'buffer-read-only)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((buffer-read-only t))
     (condition-case err (apply #'transpose-regions '(1 3 3 5))
       (error (car err)))))
 'buffer-read-only)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((buffer-read-only t))
     (condition-case err (apply #'transpose-regions '(1 8 2 3))
       (error (car err)))))
 'error)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((buffer-read-only t))
     (condition-case err (apply #'transpose-regions '(0 2 4 6))
       (error (car err)))))
 'args-out-of-range)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((qa240-events nil) (inhibit-modification-hooks t)
	(before-change-functions
	 (list (lambda (&rest args) (push args qa240-events))))
	(after-change-functions
	 (list (lambda (&rest args) (push args qa240-events)))))
     (transpose-regions 1 3 5 8) (list (buffer-string) qa240-events)))
 '("XYZ--ab" nil))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((before-change-functions
	 (list (lambda (&rest _args) (error "stop")))))
     (list
      (condition-case nil (transpose-regions 1 3 5 8) (error 'caught))
      (buffer-string))))
 '(caught "ab--XYZ"))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((after-change-functions
	 (list (lambda (&rest _args) (error "stop")))))
     (list
      (condition-case nil (transpose-regions 1 3 5 8) (error 'caught))
      (buffer-string))))
 '(caught "XYZ--ab"))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 2)
   (put-text-property 1 3 'face 'bold)
   (put-text-property 5 8 'face 'italic) (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let ((marker (copy-marker 2)))
     (transpose-regions 1 3 5 8 nil)
     (let
	 ((changed
	   (list (buffer-string) (point) (marker-position marker))))
       (setq buffer-undo-list (primitive-undo 1 buffer-undo-list))
       (list changed (buffer-string) (point) (marker-position marker)))))
 '((#("XYZ--ab" 0 3 (face italic) 5 7 (face bold)) 7 7)
   #("ab--XYZ" 0 2 (face bold) 4 7 (face italic)) 1 1))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 2)
   (put-text-property 1 3 'face 'bold)
   (put-text-property 5 8 'face 'italic) (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let ((marker (copy-marker 2)))
     (transpose-regions 1 3 5 8 t)
     (let
	 ((changed
	   (list (buffer-string) (point) (marker-position marker))))
       (setq buffer-undo-list (primitive-undo 1 buffer-undo-list))
       (list changed (buffer-string) (point) (marker-position marker)))))
 '((#("XYZ--ab" 0 3 (face italic) 5 7 (face bold)) 2 2)
   #("ab--XYZ" 0 2 (face bold) 4 7 (face italic)) 1 1))

(qa
 (with-temp-buffer
   (insert "abcdef")
   (let ((payload (list 1)))
     (put-text-property 1 3 'qa240 payload)
     (put-text-property 5 7 'qa240 payload)
     (transpose-regions 1 3 5 7) (garbage-collect)
     (list (buffer-string) (eq (get-text-property 1 'qa240) payload)
	   (eq (get-text-property 5 'qa240) payload))))
 '(#("efcdab" 0 2 (qa240 (1)) 4 6 (qa240 (1))) t t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (put-text-property 1 8 'face nil)
   (buffer-enable-undo) (setq buffer-undo-list nil)
   (transpose-regions 1 3 5 8)
   (list (object-intervals (buffer-string)) buffer-undo-list))
 '(((0 3 (face nil)) (3 5 (face nil)) (5 7 (face nil)))
   ((nil face nil 1 . 8) (1 . 8) (#("ab--XYZ" 0 7 (face nil)) . -1))))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 1) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("XYZ--ab" 1 t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 2) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("XYZ--ab" 1 t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 4) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("XYZ--ab" 1 t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 5) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("XYZ--ab" 8 t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 7) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("XYZ--ab" 8 t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (goto-char 8) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("XYZ--ab" 8 t))

(qa
 (with-temp-buffer
   (insert "αβ--日本語") (goto-char 1) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("日本語--αβ" 1 t))

(qa
 (with-temp-buffer
   (insert "αβ--日本語") (goto-char 2) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("日本語--αβ" 1 t))

(qa
 (with-temp-buffer
   (insert "αβ--日本語") (goto-char 4) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("日本語--αβ" 1 t))

(qa
 (with-temp-buffer
   (insert "αβ--日本語") (goto-char 5) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("日本語--αβ" 8 t))

(qa
 (with-temp-buffer
   (insert "αβ--日本語") (goto-char 7) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("日本語--αβ" 8 t))

(qa
 (with-temp-buffer
   (insert "αβ--日本語") (goto-char 8) (insert "!") (delete-char -1)
   (let ((size (gap-size)))
     (transpose-regions 1 3 5 8)
     (list (buffer-string) (gap-position) (= size (gap-size)))))
 '("日本語--αβ" 8 t))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (when nil (put-text-property 1 8 'face 'italic))
   (let
       ((before-change-functions
	 (list
	  (lambda (&rest _args) (put-text-property 1 8 'face 'bold)))))
     (transpose-regions 1 3 5 8)
     (list (buffer-substring-no-properties 1 8)
	   (mapcar (lambda (p) (get-text-property p 'face))
		   (number-sequence 1 7)))))
 '("XYZ--ab" (nil nil nil nil nil nil nil)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (when t (put-text-property 1 8 'face 'italic))
   (let
       ((before-change-functions
	 (list
	  (lambda (&rest _args) (put-text-property 1 8 'face 'bold)))))
     (transpose-regions 1 3 5 8)
     (list (buffer-substring-no-properties 1 8)
	   (mapcar (lambda (p) (get-text-property p 'face))
		   (number-sequence 1 7)))))
 '("XYZ--ab" (bold bold bold bold bold bold bold)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ") (put-text-property 4 5 'read-only t)
   (list
    (condition-case err (transpose-regions 1 3 5 7) (error (car err)))
    (buffer-substring-no-properties 1 8)))
 '(text-read-only "ab--XYZ"))

(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 65 128 45 45 255 254 66))
   (buffer-enable-undo) (setq buffer-undo-list nil) (goto-char 2)
   (let ((marker (copy-marker 2)))
     (transpose-regions 1 3 5 8 nil)
     (let
	 ((changed
	   (list (string-to-list (buffer-string)) (point)
		 (marker-position marker)
		 (multibyte-string-p (caar (cdr buffer-undo-list))))))
       (primitive-undo 1 buffer-undo-list)
       (list changed (string-to-list (buffer-string))
	     (multibyte-string-p (buffer-string))))))
 '(((255 254 66 45 45 65 128) 7 7 nil) (65 128 45 45 255 254 66) nil))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((markers (mapcar #'copy-marker '(1 3 5 8))))
     (apply #'transpose-regions (append markers (list nil)))
     (list (buffer-string) (mapcar #'marker-position markers))))
 '("XYZ--ab" (6 4 1 8)))

(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 65 128 45 45 255 254 66))
   (buffer-enable-undo) (setq buffer-undo-list nil) (goto-char 2)
   (let ((marker (copy-marker 2)))
     (transpose-regions 1 3 5 8 t)
     (let
	 ((changed
	   (list (string-to-list (buffer-string)) (point)
		 (marker-position marker)
		 (multibyte-string-p (caar (cdr buffer-undo-list))))))
       (primitive-undo 1 buffer-undo-list)
       (list changed (string-to-list (buffer-string))
	     (multibyte-string-p (buffer-string))))))
 '(((255 254 66 45 45 65 128) 2 2 nil) (65 128 45 45 255 254 66) nil))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((markers (mapcar #'copy-marker '(1 3 5 8))))
     (apply #'transpose-regions (append markers (list t)))
     (list (buffer-string) (mapcar #'marker-position markers))))
 '("XYZ--ab" (1 3 5 8)))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((deactivate-mark nil) (qa240-events nil)
	(before-change-functions
	 (list
	  (lambda (&rest _args)
	    (push (list 'before deactivate-mark) qa240-events))))
	(after-change-functions
	 (list
	  (lambda (&rest _args)
	    (push (list 'after deactivate-mark) qa240-events)))))
     (transpose-regions 1 3 5 8 nil)
     (list deactivate-mark (reverse qa240-events))))
 '(t ((before nil) (after t))))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((deactivate-mark nil) (qa240-events nil)
	(before-change-functions
	 (list
	  (lambda (&rest _args)
	    (push (list 'before deactivate-mark) qa240-events))))
	(after-change-functions
	 (list
	  (lambda (&rest _args)
	    (push (list 'after deactivate-mark) qa240-events)))))
     (transpose-regions 1 3 5 8 t)
     (list deactivate-mark (reverse qa240-events))))
 '(t ((before nil) (after t))))

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((deactivate-mark nil))
     (transpose-regions 1 1 1 3) deactivate-mark))
 'nil)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((deactivate-mark nil)
	(before-change-functions
	 (list (lambda (&rest _args) (error "stop")))))
     (condition-case nil (transpose-regions 1 3 5 8) (error nil))
     deactivate-mark))
 'nil)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let
       ((deactivate-mark nil)
	(after-change-functions
	 (list (lambda (&rest _args) (error "stop")))))
     (condition-case nil (transpose-regions 1 3 5 8) (error nil))
     deactivate-mark))
 't)

(qa
 (with-temp-buffer
   (insert "ab--XYZ")
   (let ((deactivate-mark nil) (inhibit-modification-hooks t))
     (transpose-regions 1 3 5 8) deactivate-mark))
 'nil)


(princ (qa--done)) (terpri)
