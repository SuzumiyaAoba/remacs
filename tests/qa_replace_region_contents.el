;;; -*- lexical-binding: t; -*-
;; Issue #235: minimal edits preserve equal islands, markers, properties,
;; point, undo and modification state. Expected values verified with GNU 31.1.
(defun qa235-positions (old new &optional seconds costs)
  (let ((positions (number-sequence 1 (1+ (length old)))))
    (list
     (mapcar (lambda (pos) (with-temp-buffer (insert old) (goto-char pos)
                (replace-region-contents 1 (point-max) new seconds costs) (point))) positions)
     (mapcar (lambda (mode) (with-temp-buffer (insert old)
                (let ((markers (mapcar (lambda (pos) (copy-marker pos mode)) positions)))
                  (replace-region-contents 1 (point-max) new seconds costs)
                  (list (buffer-string) (mapcar #'marker-position markers))))) '(nil t)))))
(defun qa235-properties (inherit styled)
  (with-temp-buffer
    (insert (propertize "abcdef" 'face 'bold))
    (replace-region-contents 1 7 (if styled (propertize "abXdeY" 'face 'italic) "abXdeY") nil nil inherit)
    (mapcar (lambda (pos) (get-text-property pos 'face)) '(1 2 3 4 5 6))))
(defvar qa235-events nil)

(qa (qa235-positions "abcdef" "abcdeX")
    '((1 2 3 4 5 6 7)
      (("abcdeX" (1 2 3 4 5 6 7)) ("abcdeX" (1 2 3 4 5 6 7)))))

(qa (qa235-positions "abcdefgh" "abXdeYgh")
    '((1 2 3 4 5 6 7 8 9)
      (("abXdeYgh" (1 2 3 4 5 6 7 8 9))
       ("abXdeYgh" (1 2 3 4 5 6 7 8 9)))))

(qa (qa235-positions "0123456789" "01X345Y789")
    '((1 2 3 4 5 6 7 8 9 10 11)
      (("01X345Y789" (1 2 3 4 5 6 7 8 9 10 11))
       ("01X345Y789" (1 2 3 4 5 6 7 8 9 10 11)))))

(qa (qa235-positions "abcdef" "XY")
    '((1 3 3 3 3 3 3) (("XY" (1 1 1 1 1 1 3)) ("XY" (1 1 1 1 1 1 3)))))

(qa (qa235-positions "abc" "")
    '((1 1 1 1) (("" (1 1 1 1)) ("" (1 1 1 1)))))

(qa (qa235-positions "" "abc") '((4) (("abc" (1)) ("abc" (4)))))

(qa (qa235-positions "" "") '((1) (("" (1)) ("" (1)))))

(qa (qa235-positions "abc" "abc")
    '((1 2 3 4) (("abc" (1 2 3 4)) ("abc" (1 2 3 4)))))

(qa (qa235-positions "ab" "ba")
    '((1 1 3) (("ba" (1 1 2)) ("ba" (1 1 3)))))

(qa (qa235-positions "abba" "baab")
    '((1 1 1 2 5) (("baab" (1 1 1 2 3)) ("baab" (1 1 1 2 5)))))

(qa (qa235-positions "abcab" "bacba")
    '((1 1 2 2 4 6) (("bacba" (1 1 2 2 3 5)) ("bacba" (1 1 2 2 4 6)))))

(qa (qa235-positions "aaa" "aaaa")
    '((1 2 3 5) (("aaaa" (1 2 3 4)) ("aaaa" (1 2 3 5)))))

(qa (qa235-positions "aaaa" "aaa")
    '((1 2 3 4 4) (("aaa" (1 2 3 4 4)) ("aaa" (1 2 3 4 4)))))

(qa (qa235-positions "αβγδεζ" "αXγδεYζ")
    '((1 2 3 4 5 7 8)
      (("αXγδεYζ" (1 2 3 4 5 6 8)) ("αXγδεYζ" (1 2 3 4 5 7 8)))))

(qa (qa235-positions "abc" "XabcY")
    '((2 3 4 6) (("XabcY" (1 3 4 5)) ("XabcY" (2 3 4 6)))))

(qa (qa235-positions "abc" "ac")
    '((1 2 2 3) (("ac" (1 2 2 3)) ("ac" (1 2 2 3)))))

(qa (qa235-positions "abcde" "aXcYZe" 0 nil)
    '((1 7 7 7 7 7)
      (("aXcYZe" (1 1 1 1 1 7)) ("aXcYZe" (1 1 1 1 1 7)))))

(qa (qa235-positions "abcde" "aXcYZe" nil 0)
    '((1 7 7 7 7 7)
      (("aXcYZe" (1 1 1 1 1 7)) ("aXcYZe" (1 1 1 1 1 7)))))

(qa (qa235-positions "abcde" "aXcYZe" nil -1)
    '((1 2 3 4 6 7)
      (("aXcYZe" (1 2 3 4 6 7)) ("aXcYZe" (1 2 3 4 6 7)))))

(qa (qa235-positions "abcde" "aXcYZe" nil 1)
    '((1 2 3 4 6 7)
      (("aXcYZe" (1 2 3 4 6 7)) ("aXcYZe" (1 2 3 4 6 7)))))

(qa (qa235-positions "abcde" "aXcYZe" nil 2)
    '((1 2 3 4 6 7)
      (("aXcYZe" (1 2 3 4 6 7)) ("aXcYZe" (1 2 3 4 6 7)))))

(qa (qa235-positions "abcde" "aXcYZe" -1 nil)
    '((1 7 7 7 7 7)
      (("aXcYZe" (1 1 1 1 1 7)) ("aXcYZe" (1 1 1 1 1 7)))))

(qa (qa235-properties nil nil) '(bold bold nil bold bold nil))

(qa (qa235-properties nil t) '(bold bold italic bold bold italic))

(qa (qa235-properties t nil) '(bold bold nil bold bold nil))

(qa (qa235-properties t t) '(bold bold italic bold bold italic))

(qa
 (with-temp-buffer
   (insert "abcdef") (goto-char 4) (buffer-enable-undo)
   (set-buffer-modified-p nil)
   (let
       ((marker (copy-marker 3)) (tick (buffer-modified-tick))
	(chars (buffer-chars-modified-tick)) (qa235-events nil)
	(before-change-functions
	 (list
	  (lambda (&rest args) (push (cons 'before args) qa235-events))))
	(after-change-functions
	 (list
	  (lambda (&rest args) (push (cons 'after args) qa235-events)))))
     (list
      (replace-region-contents 1 7 (propertize "abcdef" 'face 'bold))
      (point) (marker-position marker) (buffer-modified-p)
      buffer-undo-list (= tick (buffer-modified-tick))
      (= chars (buffer-chars-modified-tick))
      (object-intervals (buffer-string)) (reverse qa235-events))))
 '(t 4 3 nil nil t t nil ((before 1 7) (after 1 7 6))))

(qa
 (with-temp-buffer
   (insert "abcdefgh") (goto-char 4) (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let
       ((qa235-events nil) (tick (buffer-modified-tick))
	(before-change-functions
	 (list
	  (lambda (&rest args) (push (cons 'before args) qa235-events))))
	(after-change-functions
	 (list
	  (lambda (&rest args) (push (cons 'after args) qa235-events)))))
     (replace-region-contents 1 9 "abXdeYgh")
     (list (buffer-string) (point) (- (buffer-modified-tick) tick)
	   (reverse qa235-events)
	   (mapcar
	    (lambda (entry)
	      (if (consp entry)
		  (if (stringp (car entry))
		      (cons (substring-no-properties (car entry))
			    (cdr entry))
		    entry)
		entry))
	    buffer-undo-list))))
 '("abXdeYgh" 4 4 ((before 1 9) (after 1 9 8))
   (("c" . -3) (4 . 5) ("f" . 6) (7 . 8) 4)))

(qa
 (with-temp-buffer
   (insert "abcdef")
   (let
       ((inhibit-modification-hooks t) (qa235-events nil)
	(before-change-functions
	 (list (lambda (&rest args) (push args qa235-events))))
	(after-change-functions
	 (list (lambda (&rest args) (push args qa235-events)))))
     (replace-region-contents 1 7 "abXdeY")
     (list (buffer-string) qa235-events)))
 '("abXdeY" nil))

(qa
 (with-temp-buffer
   (insert "abcdef") (goto-char 3)
   (let (seen)
     (replace-region-contents 2 6
			      (lambda nil
				(setq seen
				      (list (point-min) (point-max)
					    (buffer-string)))
				(goto-char (point-max)) "bXYe"))
     (list seen (point) (point-min) (point-max) (buffer-string))))
 '((2 6 "bcde") 3 1 7 "abXYef"))

(qa
 (with-temp-buffer
   (insert "abcdef") (narrow-to-region 2 6) (goto-char 3)
   (let
       ((error
	 (condition-case nil
	     (replace-region-contents 3 5
				      (lambda nil (goto-char 5)
					(error "fail")))
	   (error 'caught))))
     (list error (point) (point-min) (point-max) (buffer-string))))
 '(caught 3 2 6 "bcde"))

(qa
 (let ((source (generate-new-buffer " *qa235-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert "0abXdeY9") (narrow-to-region 2 8))
	 (with-temp-buffer
	   (insert "abcdef") (replace-region-contents 1 7 source)
	   (buffer-string)))
     (kill-buffer source)))
 '"abXdeY")

(qa
 (let ((source (generate-new-buffer " *qa235-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert "0abXdeY9") (narrow-to-region 2 8))
	 (with-temp-buffer
	   (insert "abcdef")
	   (replace-region-contents 1 7 (vector source 8 2))
	   (buffer-string)))
     (kill-buffer source)))
 '"abXdeY")

(qa
 (with-temp-buffer
   (insert "abcdef") (goto-char 4) (narrow-to-region 2 6)
   (replace-region-contents 5 3 "XY")
   (list (point-min) (point-max) (point) (buffer-string)))
 '(2 6 5 "bXYe"))

(qa
 (with-temp-buffer
   (insert (propertize "abcdefgh" 'face 'bold)) (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let ((m (copy-marker 5)))
     (replace-region-contents 1 9
			      (propertize "abXdeYgh" 'face 'italic))
     (setq buffer-undo-list (primitive-undo 1 buffer-undo-list))
     (list (buffer-substring-no-properties 1 9)
	   (mapcar (lambda (p) (get-text-property p 'face))
		   '(1 2 3 4 5 6 7 8))
	   (marker-position m))))
 '("abcdefgh" (bold bold bold bold bold bold bold bold) 5))

(qa
 (with-temp-buffer
   (insert "abcdefgh")
   (let ((ov (make-overlay 4 6)))
     (overlay-put ov 'evaporate t)
     (replace-region-contents 1 9 "abXdeYgh")
     (list (overlay-start ov) (overlay-end ov)
	   (eq (overlay-buffer ov) (current-buffer)))))
 '(4 6 t))

(qa
 (with-temp-buffer
   (insert "abcd")
   (let ((ov (make-overlay 1 5)))
     (overlay-put ov 'evaporate t) (replace-region-contents 1 5 "XY")
     (list (overlay-start ov) (overlay-end ov)
	   (eq (overlay-buffer ov) (current-buffer)))))
 '(1 3 t))

(qa
 (with-temp-buffer
   (insert "abc")
   (let ((buffer-read-only t))
     (condition-case err (replace-region-contents 1 4 "abc")
       (error (car err)))))
 'buffer-read-only)

(qa
 (with-temp-buffer
   (insert "abc")
   (let ((buffer-read-only t))
     (condition-case err (replace-region-contents 1 4 "abX")
       (error (car err)))))
 'buffer-read-only)

(qa
 (with-temp-buffer
   (insert "abc")
   (let ((buffer-read-only t))
     (condition-case err (replace-region-contents 1 4 "")
       (error (car err)))))
 'buffer-read-only)

(qa
 (with-temp-buffer
   (let ((buffer-read-only t))
     (replace-region-contents 1 1 "" 'bad 'bad)))
 't)

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 42) (error err)))
 '(wrong-type-argument (or string buffer vector) 42))

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 'unknown-source)
     (error err)))
 '(wrong-type-argument (or string buffer vector) unknown-source))

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 []) (error err)))
 '(args-out-of-range [] 2))

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 [42 1 2])
     (error err)))
 '(wrong-type-argument bufferp 42))

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 "abX" nil 'bad)
     (error err)))
 '(wrong-type-argument integerp bad))

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 "abX" 'bad)
     (error err)))
 '(error "Invalid time specification"))

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (replace-region-contents 1 4 (current-buffer))
     (error err)))
 '(error "Cannot replace a buffer with itself"))

(qa
 (with-temp-buffer
   (insert "abcdef")
   (let ((payload (list 1)))
     (put-text-property 2 6 'qa235 payload)
     (replace-region-contents 1 7 "abXdeY") (garbage-collect)
     (list (eq (get-text-property 2 'qa235) payload)
	   (get-text-property 3 'qa235)
	   (eq (get-text-property 4 'qa235) payload))))
 '(t nil t))

(qa
 (with-temp-buffer
   (insert "abc") (replace-region-contents 1 4 "abX" 10)
   (buffer-string))
 '"abX")

(qa
 (with-temp-buffer
   (insert (make-string 100000 97))
   (let ((marker (copy-marker 10)))
     (list
      (replace-region-contents 1 (point-max)
			       (concat (make-string 100000 97) "b") -1)
      (marker-position marker) (point) (buffer-size))))
 '(nil 1 100002 100001))

(qa
 (with-temp-buffer
   (insert "abcdefgh") (goto-char 1) (search-forward "de")
   (replace-region-contents 1 9 "abXXdeYgh" nil nil t)
   (list (match-beginning 0) (match-end 0)))
 '(5 7))

(qa
 (with-temp-buffer
   (insert "abcdefgh") (goto-char 1) (search-forward "de")
   (replace-region-contents 1 9 "abXXdeYgh" nil nil nil)
   (list (match-beginning 0) (match-end 0)))
 '(4 6))

(qa
 (let ((source (generate-new-buffer " *qa235-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert (propertize "abXdeY" 'face 'italic)))
	 (with-temp-buffer
	   (insert (propertize "abcdef" 'face 'bold))
	   (replace-region-contents 1 7 source)
	   (mapcar (lambda (p) (get-text-property p 'face))
		   '(1 2 3 4 5 6))))
     (kill-buffer source)))
 '(bold bold italic bold bold italic))

(qa
 (with-temp-buffer
   (set-buffer-multibyte nil) (insert (unibyte-string 65 128 255 66))
   (goto-char 3)
   (let ((m (copy-marker 2)))
     (replace-region-contents 1 5 (unibyte-string 65 128 254 66))
     (list (string-to-list (buffer-string)) (point)
	   (marker-position m))))
 '((65 128 254 66) 3 2))

(qa
 (with-temp-buffer
   (insert "abcdefgh")
   (let ((ov (make-overlay 3 7 nil t t)))
     (replace-region-contents 1 9 "abXdeYgh")
     (list (overlay-start ov) (overlay-end ov))))
 '(3 7))

(qa
 (with-temp-buffer
   (insert "abcdefgh") (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let
       ((before-change-functions
	 (list
	  (lambda (beg end)
	    (let ((buffer-undo-list t))
	      (put-text-property beg end 'face 'italic))))))
     (replace-region-contents 1 9 "abXdeYgh")
     (list (get-text-property 0 'face (caar buffer-undo-list))
	   (mapcar (lambda (p) (get-text-property p 'face))
		   '(1 2 3 4 5 6 7 8)))))
 '(italic (italic italic nil italic italic nil italic italic)))

(qa
 (with-temp-buffer
   (insert "abcdef")
   (let
       ((qa235-events nil)
	(before-change-functions
	 (list
	  (lambda (&rest args) (push args qa235-events)
	    (error "stop")))))
     (list
      (condition-case nil (replace-region-contents 1 7 "abXdeY")
	(error 'caught))
      (buffer-string) qa235-events)))
 '(caught "abcdef" ((1 7))))

(qa
 (with-temp-buffer
   (insert "abcdef")
   (let
       ((qa235-events nil)
	(after-change-functions
	 (list
	  (lambda (&rest args) (push args qa235-events)
	    (error "stop")))))
     (list
      (condition-case nil (replace-region-contents 1 7 "abXdeY")
	(error 'caught))
      (buffer-string) qa235-events)))
 '(caught "abXdeY" ((1 7 6))))

(qa
 (with-temp-buffer
   (set-buffer-multibyte nil) (insert (unibyte-string 65 255 66))
   (goto-char 3)
   (let ((tick (buffer-modified-tick)))
     (replace-region-contents 1 4 (string 65 255 66))
     (list (string-to-list (buffer-string)) (point)
	   (- (buffer-modified-tick) tick))))
 '((65 255 66) 3 2))

(qa
 (with-temp-buffer
   (insert (unibyte-string 65 255 66)) (goto-char 3)
   (let ((tick (buffer-modified-tick)))
     (replace-region-contents 1 4 (unibyte-string 65 255 66))
     (list (string-to-list (buffer-string)) (point)
	   (= (buffer-modified-tick) tick))))
 '((65 4194303 66) 3 t))

(qa (qa235-positions "abba" "baab" nil (expt 2 200))
    '((1 1 1 2 5) (("baab" (1 1 1 2 3)) ("baab" (1 1 1 2 5)))))

(qa (qa235-positions "abba" "baab" nil (- (expt 2 200)))
    '((1 1 1 3 5) (("baab" (1 1 1 2 4)) ("baab" (1 1 1 3 5)))))

(qa
 (with-temp-buffer
   (insert "abcdefgh") (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let
       ((markers
	 (mapcar (lambda (p) (copy-marker p nil))
		 (number-sequence 1 9))))
     (replace-region-contents 1 9 "aXh")
     (let
	 ((changed
	   (list (mapcar #'marker-position markers)
		 (mapcar
		  (lambda (entry)
		    (if (and (consp entry) (markerp (car entry)))
			'marker-adjustment
		      entry))
		  buffer-undo-list))))
       (primitive-undo 1 buffer-undo-list)
       (list changed (buffer-string)
	     (mapcar #'marker-position markers)))))
 '(((1 2 2 2 2 2 2 3 4) (("bcdefg" . 2) (8 . 9) 9)) "abcdefgh"
   (1 2 2 2 2 2 2 8 9)))

(qa
 (with-temp-buffer
   (insert "abcdefgh") (buffer-enable-undo)
   (setq buffer-undo-list nil)
   (let
       ((markers
	 (mapcar (lambda (p) (copy-marker p t)) (number-sequence 1 9))))
     (replace-region-contents 1 9 "aXh")
     (let
	 ((changed
	   (list (mapcar #'marker-position markers)
		 (mapcar
		  (lambda (entry)
		    (if (and (consp entry) (markerp (car entry)))
			'marker-adjustment
		      entry))
		  buffer-undo-list))))
       (primitive-undo 1 buffer-undo-list)
       (list changed (buffer-string)
	     (mapcar #'marker-position markers)))))
 '(((1 2 2 2 2 2 2 3 4) (("bcdefg" . 2) (8 . 9) 9)) "abcdefgh"
   (1 8 8 8 8 8 8 8 9)))


(princ (qa--done)) (terpri)
