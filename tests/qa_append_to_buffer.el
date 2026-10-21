;;; -*- lexical-binding: t; -*-
;; Issue #242: property-aware buffer transfer and GNU excursion/hook behavior.
(defvar qa242-seen nil)
(defun qa242-transfer (start end source-multibyte target-multibyte same &optional narrow)
  (let ((source (generate-new-buffer " *qa242-source*"))
        (destination (generate-new-buffer " *qa242-target*")))
    (unwind-protect
        (progn
          (with-current-buffer source
            (set-buffer-multibyte source-multibyte)
            (insert (if source-multibyte "é日本" (unibyte-string 233 255 65)))
            (put-text-property 1 4 'face 'bold)
            (when narrow (narrow-to-region 2 4)) (goto-char (point-min)))
          (with-current-buffer destination
            (set-buffer-multibyte target-multibyte) (insert "[]") (goto-char 2))
          (with-current-buffer source
            (let* ((target (if same source destination))
                   (result (append-to-buffer target start end)))
              (list result (eq (current-buffer) source) (point)
                    (with-current-buffer target
                      (list (string-to-list (buffer-string)) (multibyte-string-p (buffer-string)) (point)
                            (mapcar (lambda (p) (get-text-property p 'face))
                                    (number-sequence (point-min) (1- (point-max))))))))))
      (kill-buffer source) (kill-buffer destination))))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert (propertize "abc" 'face 'bold))
	 (append-to-buffer target 1 4)
	 (with-current-buffer target
	   (list (buffer-string)
		 (mapcar (lambda (p) (get-text-property p 'face))
			 '(1 2 3)))))
     (kill-buffer target)))
 '(#("abc" 0 3 (face bold)) (bold bold bold)))

(qa (qa242-transfer 1 4 nil nil nil)
    '(nil t 1 ((91 233 255 65 93) nil 5 (nil bold bold bold nil))))

(qa (qa242-transfer 1 4 nil t nil)
    '(nil t 1
	  ((91 4194281 4194303 65 93) t 5 (nil bold bold bold nil))))

(qa (qa242-transfer 1 4 t nil nil)
    '(nil t 1 ((91 233 229 44 93) nil 5 (nil bold bold bold nil))))

(qa (qa242-transfer 1 4 t t nil)
    '(nil t 1 ((91 233 26085 26412 93) t 5 (nil bold bold bold nil))))

(qa (qa242-transfer nil nil t t nil t)
    '(nil t 2 ((91 26085 26412 93) t 4 (nil bold bold nil))))

(qa (qa242-transfer 4 2 t t nil t)
    '(nil t 2 ((91 26085 26412 93) t 4 (nil bold bold nil))))

(qa (qa242-transfer 2 4 t t nil t)
    '(nil t 2 ((91 26085 26412 93) t 4 (nil bold bold nil))))

(qa (qa242-transfer 1 4 t t t)
    '(nil t 1
	  ((233 26085 26412 233 26085 26412) t 1
	   (bold bold bold bold bold bold))))

(qa (qa242-transfer nil nil nil nil t)
    '(nil t 1
	  ((233 255 65 233 255 65) nil 1
	   (bold bold bold bold bold bold))))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert (propertize "abc" 'face 'bold))
	 (with-current-buffer target
	   (insert "[]") (goto-char 2) (buffer-enable-undo)
	   (setq buffer-undo-list nil) (set-buffer-modified-p nil))
	 (let ((qa242-seen nil))
	   (with-current-buffer target
	     (setq-local before-change-functions
			 (list
			  (lambda (&rest args)
			    (push (list 'before args (point))
				  qa242-seen))))
	     (setq-local after-change-functions
			 (list
			  (lambda (&rest args)
			    (push
			     (list 'after args (point)
				   (get-text-property 2 'face))
			     qa242-seen)))))
	   (let
	       ((tick
		 (with-current-buffer target (buffer-modified-tick))))
	     (append-to-buffer target 1 4)
	     (list (reverse qa242-seen)
		   (with-current-buffer target
		     (list (buffer-string) (point)
			   (- (buffer-modified-tick) tick)
			   buffer-undo-list))))))
     (kill-buffer target)))
 '(((before (2 2) 2) (after (2 5 0) 5 bold))
   (#("[abc]" 1 4 (face bold)) 5 2 ((2 . 5) (t . 0)))))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert (propertize "abc" 'face 'bold)) (goto-char 2)
	 (with-current-buffer target
	   (insert "[]") (goto-char 2)
	   (setq-local after-change-functions
		       (list (lambda (&rest _args) (error "stop")))))
	 (list
	  (condition-case nil (append-to-buffer target 1 4)
	    (error 'caught))
	  (point)
	  (with-current-buffer target
	    (list (buffer-string) (point) (get-text-property 2 'face)))))
     (kill-buffer target)))
 '(caught 2 (#("[abc]" 1 4 (face bold)) 5 bold)))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert "abc")
	 (with-current-buffer target (setq buffer-read-only t))
	 (condition-case err (append-to-buffer target 'bad 'bad)
	   (error (car err))))
     (kill-buffer target)))
 'buffer-read-only)

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (with-current-buffer target (setq buffer-read-only t))
	 (condition-case err (append-to-buffer target 1 1)
	   (error (car err))))
     (kill-buffer target)))
 'buffer-read-only)

(qa
 (with-temp-buffer
   (insert "abc")
   (condition-case err (append-to-buffer 42 'bad 'bad) (error err)))
 '(wrong-type-argument stringp 42))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert "abc") (narrow-to-region 2 4)
	 (condition-case err (append-to-buffer target 1 4)
	   (error err)))
     (kill-buffer target)))
 '(args-out-of-range 1 4))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert (propertize "abc" 'face 'bold))
	 (append-to-buffer target (copy-marker 4) (copy-marker 2))
	 (with-current-buffer target (list (buffer-string) (point))))
     (kill-buffer target)))
 '(#("bc" 0 2 (face bold)) 3))

(qa
 (let ((name " *qa242-created*"))
   (unwind-protect
       (with-temp-buffer
	 (insert "abc")
	 (let
	     ((error
	       (condition-case err (append-to-buffer name 0 2)
		 (error (car err)))))
	   (list error (not (null (get-buffer name)))
		 (with-current-buffer name (buffer-string)))))
     (when (get-buffer name) (kill-buffer name))))
 '(args-out-of-range t ""))

(qa
 (with-temp-buffer
   (insert "[]") (goto-char 2) (set-buffer-modified-p nil)
   (let
       ((source (generate-new-buffer " *qa242-empty*"))
	(qa242-seen nil) (deactivate-mark nil)
	(before-change-functions
	 (list
	  (lambda (&rest args) (push (cons 'before args) qa242-seen))))
	(after-change-functions
	 (list
	  (lambda (&rest args) (push (cons 'after args) qa242-seen)))))
     (unwind-protect
	 (let ((tick (buffer-modified-tick)))
	   (insert-buffer-substring source)
	   (list (reverse qa242-seen) (= tick (buffer-modified-tick))
		 (point) deactivate-mark))
       (kill-buffer source))))
 '(((after 2 2 0)) t 2 nil))

(qa
 (let ((source (generate-new-buffer " *qa242-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert (propertize "abc" 'face 'bold)))
	 (with-temp-buffer
	   (let
	       ((before-change-functions
		 (list
		  (lambda (&rest _args)
		    (with-current-buffer source
		      (put-text-property 1 4 'face 'italic))))))
	     (insert-buffer-substring source)
	     (list (buffer-string) (get-text-property 1 'face)))))
     (kill-buffer source)))
 '(#("abc" 0 3 (face italic)) italic))

(qa
 (let ((target (generate-new-buffer " *qa242-target*")))
   (unwind-protect
       (with-temp-buffer
	 (insert "abc")
	 (with-current-buffer target
	   (insert "[]") (goto-char 2)
	   (setq-local before-change-functions
		       (list (lambda (&rest _args) (error "stop")))))
	 (list
	  (condition-case nil (append-to-buffer target 1 4)
	    (error 'caught))
	  (point)
	  (with-current-buffer target (list (buffer-string) (point)))))
     (kill-buffer target)))
 '(caught 4 ("[]" 2)))

(qa
 (save-window-excursion
   (delete-other-windows)
   (let
       ((source (generate-new-buffer " *qa242-source*"))
	(target (generate-new-buffer " *qa242-target*")))
     (unwind-protect
	 (progn
	   (with-current-buffer target (insert "[]") (goto-char 2))
	   (let
	       ((window
		 (if nil (selected-window) (split-window-right))))
	     (set-window-buffer window target)
	     (set-window-point window 2)
	     (with-current-buffer source
	       (insert (propertize "abc" 'face 'bold)) (goto-char 2)
	       (append-to-buffer target 1 4)
	       (list (point) (window-point window)
		     (with-current-buffer target
		       (list (point) (buffer-string)))))))
       (kill-buffer source) (kill-buffer target))))
 '(2 5 (5 #("[abc]" 1 4 (face bold)))))

(qa
 (save-window-excursion
   (delete-other-windows)
   (let
       ((source (generate-new-buffer " *qa242-source*"))
	(target (generate-new-buffer " *qa242-target*")))
     (unwind-protect
	 (progn
	   (with-current-buffer target (insert "[]") (goto-char 2))
	   (let
	       ((window (if t (selected-window) (split-window-right))))
	     (set-window-buffer window target)
	     (set-window-point window 2)
	     (with-current-buffer source
	       (insert (propertize "abc" 'face 'bold)) (goto-char 2)
	       (append-to-buffer target 1 4)
	       (list (point) (window-point window)
		     (with-current-buffer target
		       (list (point) (buffer-string)))))))
       (kill-buffer source) (kill-buffer target))))
 '(2 5 (5 #("[abc]" 1 4 (face bold)))))


(princ (qa--done)) (terpri)
