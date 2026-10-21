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


;; Issue #278: restore topology and original internal-window identity.
(qa
(qa277-window-fixture
 (lambda (_buffer window) (split-window window nil 'right)
   (let ((saved (current-window-configuration)))
     (set-window-configuration saved)
     (let ((root (frame-root-window)))
       (list (eq (window-left-child root) window)
	     (eq (window-top-child root) window)
	     (length (window-list)))))))
'(t nil 2)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let*
       ((right (split-window window nil 'right))
	(bottom (split-window right nil 'below))
	(saved (current-window-configuration)))
     (set-window-configuration saved)
     (list (eq (window-parent window) (window-parent right))
	   (eq (window-parent right) (window-parent bottom))))))
'(nil t)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (split-window window nil 'right)
   (let
       ((root (frame-root-window))
	(saved (current-window-configuration)))
     (set-window-configuration saved)
     (list (eq root (frame-root-window))
	   (eq (window-parent window) root) (window-valid-p root)))))
'(t t t)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let*
       ((right (split-window window nil 'right))
	(bottom (split-window right nil 'below))
	(root (frame-root-window)) (inner (window-parent right))
	(saved (current-window-configuration)))
     (delete-other-windows window)
     (let
	 ((before
	   (mapcar #'window-valid-p (list root inner right bottom))))
       (set-window-configuration saved)
       (list before
	     (mapcar #'window-valid-p (list root inner right bottom))
	     (eq root (frame-root-window))
	     (eq (window-parent inner) root)
	     (eq (window-parent right) inner)
	     (eq (window-parent bottom) inner))))))
'((nil nil nil nil) (t t t t) t t t t)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (split-window window nil 'right)
   (let
       ((root (frame-root-window))
	(saved (current-window-configuration)))
     (delete-other-windows window)
     (let*
	 ((new (split-window window nil 'below))
	  (new-root (frame-root-window)))
       (set-window-configuration saved)
       (list (eq root (frame-root-window)) (window-valid-p new-root)
	     (window-live-p new))))))
'(t nil nil)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (split-window window nil 'right)
   (let
       ((horizontal (current-window-configuration))
	(hroot (frame-root-window)))
     (delete-other-windows window) (split-window window nil 'below)
     (let
	 ((vertical (current-window-configuration))
	  (vroot (frame-root-window)))
       (set-window-configuration horizontal)
       (let ((first (eq (frame-root-window) hroot)))
	 (set-window-configuration vertical)
	 (let ((second (eq (frame-root-window) vroot)))
	   (set-window-configuration horizontal)
	   (list first second (eq (frame-root-window) hroot)
		 (eq (window-left-child (frame-root-window)) window))))))))
'(t t t t)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let*
       ((right (split-window window nil 'right))
	(left-bottom (split-window window nil 'below))
	(right-bottom (split-window right nil 'below))
	(saved (current-window-configuration)))
     (delete-other-windows window) (set-window-configuration saved)
     (list (eq (window-parent window) (window-parent left-bottom))
	   (eq (window-parent right) (window-parent right-bottom))
	   (eq (window-parent window) (window-parent right))
	   (eq (window-parent (window-parent right))
	       (frame-root-window))))))
'(t t nil t)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((right (split-window window nil 'right)))
     (split-window right nil 'below)
     (let
	 ((before (window-tree))
	  (saved (current-window-configuration)))
       (delete-other-windows window) (set-window-configuration saved)
       (equal before (window-tree))))))
't
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (split-window window nil 'right)
   (let ((root (frame-root-window)))
     (set-window-combination-limit root t)
     (let ((saved (current-window-configuration)))
       (set-window-combination-limit root nil)
       (set-window-configuration saved)
       (window-combination-limit (frame-root-window))))))
't
)


(princ (qa--done)) (terpri)
