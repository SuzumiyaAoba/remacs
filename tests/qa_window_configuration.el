;;; -*- lexical-binding: t; -*-
(defun qa277-window-fixture (fn)
  (let ((saved (current-window-configuration))
        (buffer (generate-new-buffer " *qa277-window-buffer*")))
    (unwind-protect
        (progn
          (delete-other-windows)
          (switch-to-buffer buffer)
          (insert (apply #'concat (make-list 40 "0123456789\n")))
          (goto-char 1)
          (set-window-start nil 1 t) (set-window-hscroll nil 0)
          (funcall fn buffer (selected-window)))
      (set-window-dedicated-p nil nil)
      (set-window-configuration saved)
      (when (buffer-live-p buffer) (kill-buffer buffer)))))

(qa
(qa277-window-fixture
 (lambda (buffer window) (set-window-start window 12 t)
   (set-window-hscroll window 2)
   (let ((saved (current-window-configuration)))
     (set-window-start window 23 t) (set-window-hscroll window 9)
     (set-window-configuration saved)
     (list (window-start window) (window-hscroll window)))))
'(12 2)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((saved (current-window-configuration)))
     (goto-char 12) (set-window-configuration saved)
     (list (point) (window-point window) (window-start window)))))
'(12 12 1)
)

(qa
(qa277-window-fixture
 (lambda (buffer window) (set-window-margins window 2 3)
   (let ((saved (current-window-configuration)))
     (set-window-margins window 4 5) (set-window-configuration saved)
     (window-margins window))))
'(2 . 3)
)

(qa
(qa277-window-fixture
 (lambda (buffer window) (set-window-margins window nil nil)
   (let ((saved (current-window-configuration)))
     (set-window-margins window 4 5) (set-window-configuration saved)
     (window-margins window))))
'(nil)
)

(qa
(qa277-window-fixture
 (lambda (buffer window) (set-window-margins window 0 0)
   (let ((saved (current-window-configuration)))
     (set-window-margins window 4 5) (set-window-configuration saved)
     (window-margins window))))
'(nil)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa277-other*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other (insert "01234567890123456789"))
	   (let ((second (split-window window nil 'below)))
	     (set-window-buffer second other)
	     (set-window-point second 4)
	     (let ((saved (current-window-configuration)))
	       (set-window-point second 12)
	       (set-window-configuration saved)
	       (list (window-point second)
		     (with-current-buffer other (point))))))
       (kill-buffer other)))))
'(4 12)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa277-other*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other (insert "01234567890123456789"))
	   (let ((saved (current-window-configuration)))
	     (goto-char 12) (set-buffer other) (goto-char 7)
	     (set-window-configuration saved)
	     (list (eq (current-buffer) buffer)
		   (with-current-buffer buffer (point))
		   (with-current-buffer other (point))
		   (window-point window))))
       (kill-buffer other)))))
'(t 12 7 12)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa277-other*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other (insert "01234567890123456789"))
	   (set-window-buffer window other) (set-buffer buffer)
	   (set-window-point window 4)
	   (let ((saved (current-window-configuration)))
	     (goto-char 15) (set-window-point window 12)
	     (set-window-configuration saved)
	     (list (eq (current-buffer) buffer) (point)
		   (window-point window))))
       (kill-buffer other)))))
'(t 15 4)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((second (split-window window nil 'below)))
     (set-window-point second 4)
     (let ((saved (current-window-configuration)))
       (goto-char 12) (set-window-point second 9)
       (set-window-configuration saved)
       (list (point) (window-point window) (window-point second))))))
'(12 12 4)
)

(qa
(qa277-window-fixture
 (lambda (buffer window) (set-window-start window 12 t)
   (let ((saved (current-window-configuration)))
     (goto-char 1) (insert "X") (set-window-start window 23 t)
     (set-window-configuration saved)
     (list (window-start window) (point)))))
'(13 2)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa277-other*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other (insert "01234567890123456789"))
	   (let ((second (split-window window nil 'below)))
	     (set-window-buffer second other)
	     (set-window-point second 4)
	     (let ((saved (current-window-configuration)))
	       (with-current-buffer other (goto-char 1) (insert "X"))
	       (set-window-point second 12)
	       (set-window-configuration saved) (window-point second))))
       (kill-buffer other)))))
'5
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((second (split-window window nil 'below)))
     (set-window-point second 4)
     (let ((saved (current-window-configuration)))
       (select-window second) (goto-char 12)
       (set-window-configuration saved)
       (list (eq (selected-window) window) (point)
	     (window-point window) (window-point second))))))
'(t 1 1 4)
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa277-other*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other (insert "01234567890123456789"))
	   (let ((second (split-window window nil 'below)))
	     (set-window-point second 4)
	     (let ((saved (current-window-configuration)))
	       (select-window second) (set-buffer other) (goto-char 7)
	       (set-window-configuration saved)
	       (list (eq (selected-window) window)
		     (eq (current-buffer) buffer) (point)
		     (with-current-buffer other (point))
		     (window-point second)))))
       (kill-buffer other)))))
'(t t 1 7 4)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((other (generate-new-buffer " *qa277-insertion*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other
	     (insert "01234567890123456789")
	     (setq-local window-point-insertion-type nil))
	   (let ((second (split-window window nil 'below)))
	     (set-window-buffer second other)
	     (set-window-point second 4)
	     (let ((saved (current-window-configuration)))
	       (with-current-buffer other (goto-char 4) (insert "X"))
	       (set-window-point second 12)
	       (set-window-configuration saved) (window-point second))))
       (kill-buffer other)))))
'4
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (setq-local window-point-insertion-type nil)
   (set-window-start window 12 t)
   (let ((saved (current-window-configuration)))
     (goto-char 12) (insert "X") (set-window-start window 23 t)
     (set-window-configuration saved)
     (list (window-start window) (point)))))
'(12 13)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((other (generate-new-buffer " *qa277-insertion*")))
     (unwind-protect
	 (progn
	   (with-current-buffer other
	     (insert "01234567890123456789")
	     (setq-local window-point-insertion-type t))
	   (let ((second (split-window window nil 'below)))
	     (set-window-buffer second other)
	     (set-window-point second 4)
	     (let ((saved (current-window-configuration)))
	       (with-current-buffer other (goto-char 4) (insert "X"))
	       (set-window-point second 12)
	       (set-window-configuration saved) (window-point second))))
       (kill-buffer other)))))
'5
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (setq-local window-point-insertion-type t)
   (set-window-start window 12 t)
   (let ((saved (current-window-configuration)))
     (goto-char 12) (insert "X") (set-window-start window 23 t)
     (set-window-configuration saved)
     (list (window-start window) (point)))))
'(12 13)
)


(princ (qa--done)) (terpri)
