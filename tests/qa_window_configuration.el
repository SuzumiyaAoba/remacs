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


;; Issue #279: compare visible layout, excluding point/scrolling details.
(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (goto-char 12)
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (set-window-start window 23 t)
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (set-window-hscroll window 7)
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (set-window-start window 23 t) (set-window-hscroll window 7)
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (set-window-margins window 2 3)
     (compare-window-configurations before
				    (current-window-configuration)))))
'nil
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (set-window-dedicated-p window t)
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((before (current-window-configuration)))
     (set-window-start window 12 t)
     (window-configuration-equal-p before
				   (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window) (split-window window nil 'right)
   (let ((before (current-window-configuration)))
     (set-window-combination-limit (frame-root-window) t)
     (compare-window-configurations before
				    (current-window-configuration)))))
't
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((second (split-window window nil 'below)))
     (let ((before (current-window-configuration)))
       (select-window second)
       (compare-window-configurations before
				      (current-window-configuration))))))
'nil
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa279-other*")))
     (unwind-protect
	 (let ((before (current-window-configuration)))
	   (with-current-buffer other
	     (compare-window-configurations before
					    (current-window-configuration))))
       (kill-buffer other)))))
'nil
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((other (generate-new-buffer " *qa279-other*")))
     (unwind-protect
	 (let ((before (current-window-configuration)))
	   (set-window-buffer window other)
	   (compare-window-configurations before
					  (current-window-configuration)))
       (kill-buffer other)))))
'nil
)

(qa
(qa277-window-fixture
 (lambda (buffer window)
   (let ((second (split-window window nil 'below)))
     (let ((before (current-window-configuration)))
       (delete-window second) (split-window window nil 'below)
       (compare-window-configurations before
				      (current-window-configuration))))))
't
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((right (split-window window nil 'right)))
     (split-window window nil 'below) (split-window right nil 'below)
     (let ((before (current-window-configuration)))
       (delete-other-windows window)
       (let ((bottom (split-window window nil 'below)))
	 (split-window window nil 'right)
	 (split-window bottom nil 'right))
       (compare-window-configurations before
				      (current-window-configuration))))))
'nil
)

(qa
(qa277-window-fixture
 (lambda (_buffer _window)
   (let*
       ((frame (selected-frame))
	(old (frame-parameter frame 'menu-bar-lines))
	(before (current-window-configuration)))
     (unwind-protect
	 (progn
	   (set-frame-parameter frame 'menu-bar-lines
				(if (eq old 0) 1 0))
	   (compare-window-configurations before
					  (current-window-configuration)))
       (set-frame-parameter frame 'menu-bar-lines old)))))
'nil
)

(qa
(qa277-window-fixture
 (lambda (_buffer _window)
   (let*
       ((frame (selected-frame)) (old (frame-parameter frame 'name))
	(before (current-window-configuration)))
     (unwind-protect
	 (progn
	   (set-frame-name "qa279-temporary")
	   (compare-window-configurations before
					  (current-window-configuration)))
       (set-frame-name old)))))
't
)


;; Issue #280: only factory configurations have the native opaque type.
(defun qa280-probe (kind operation)
 (let ((value (pcase kind ('real (current-window-configuration))
               ('fake (record 'window-configuration nil nil nil))
               ('uninterned (record (make-symbol "window-configuration") nil nil nil)))))
  (condition-case error-data
   (pcase operation
    ('predicates (list (window-configuration-p value) (recordp value)
                      (arrayp value) (vectorp value) (sequencep value) (symbol-name (type-of value))))
    ('frame (eq (window-configuration-frame value) (selected-frame)))
    ('length (length value))
    ('aref (type-of (aref value 0)))
    ('aref-bad-index (aref value 'bad))
    ('aset-bad-index (aset value 'bad nil))
    ('elt (elt value 0))
    ('elt-bad-index (elt value 'bad))
    ('copy (let ((copy (copy-sequence value))) (list (window-configuration-p copy) (recordp copy) (eq copy value))))
    ('print (prin1-to-string value))
    ('copy-tree (eq value (copy-tree value t)))
    ('fillarray (fillarray value nil))
    ('reverse (reverse value))
    ('nreverse (nreverse value))
    ('aset (let ((result (condition-case e (progn (aset value 0 'changed) 'changed)
                          (error (list (car e) (cadr e))))))
            (list result (window-configuration-p value) (recordp value)))))
   (error (list (car error-data) (cadr error-data))))))
(qa
(qa280-probe 'real 'predicates)
'(t nil nil nil nil "window-configuration")
)

(qa
(qa280-probe 'real 'frame)
't
)

(qa
(qa280-probe 'real 'length)
'(wrong-type-argument sequencep)
)

(qa
(qa280-probe 'real 'aref)
'(wrong-type-argument arrayp)
)

(qa
(qa280-probe 'real 'aref-bad-index)
'(wrong-type-argument fixnump)
)

(qa
(qa280-probe 'real 'aset-bad-index)
'(wrong-type-argument fixnump)
)

(qa
(qa280-probe 'real 'copy)
'(wrong-type-argument sequencep)
)

(qa
(qa280-probe 'real 'print)
'"#<window-configuration>"
)

(qa
(qa280-probe 'real 'aset)
'((wrong-type-argument arrayp) t nil)
)

(qa
(qa280-probe 'fake 'predicates)
'(nil t nil nil nil "window-configuration")
)

(qa
(qa280-probe 'fake 'frame)
'(wrong-type-argument window-configuration-p)
)

(qa
(qa280-probe 'fake 'length)
'4
)

(qa
(qa280-probe 'fake 'aref)
'symbol
)

(qa
(qa280-probe 'fake 'aref-bad-index)
'(wrong-type-argument fixnump)
)

(qa
(qa280-probe 'fake 'aset-bad-index)
'(wrong-type-argument fixnump)
)

(qa
(qa280-probe 'fake 'copy)
'(nil t nil)
)

(qa
(qa280-probe 'fake 'print)
'"#s(window-configuration nil nil nil)"
)

(qa
(qa280-probe 'fake 'aset)
'(changed nil t)
)

(qa
(qa280-probe 'uninterned 'predicates)
'(nil t nil nil nil "window-configuration")
)

(qa
(qa280-probe 'uninterned 'frame)
'(wrong-type-argument window-configuration-p)
)

(qa
(qa280-probe 'uninterned 'length)
'4
)

(qa
(qa280-probe 'uninterned 'aref)
'symbol
)

(qa
(qa280-probe 'uninterned 'aref-bad-index)
'(wrong-type-argument fixnump)
)

(qa
(qa280-probe 'uninterned 'aset-bad-index)
'(wrong-type-argument fixnump)
)

(qa
(qa280-probe 'uninterned 'copy)
'(nil t nil)
)

(qa
(qa280-probe 'uninterned 'print)
'"#s(window-configuration nil nil nil)"
)

(qa
(qa280-probe 'uninterned 'aset)
'(changed nil t)
)

(qa
(qa280-probe 'real 'elt)
'(wrong-type-argument sequencep)
)

(qa
(qa280-probe 'real 'elt-bad-index)
'(wrong-type-argument sequencep)
)

(qa
(qa280-probe 'real 'copy-tree)
't
)

(qa
(qa280-probe 'real 'reverse)
'(wrong-type-argument sequencep)
)

(qa
(qa280-probe 'real 'nreverse)
'(wrong-type-argument arrayp)
)

(qa
(let*
    ((fake (record 'window-configuration (selected-frame) nil nil))
     (copy (copy-sequence fake)))
  (list (window-configuration-p fake) (window-configuration-p copy)
	(condition-case e (window-configuration-frame fake)
	  (error (list (car e) (cadr e))))))
'(nil nil (wrong-type-argument window-configuration-p))
)

(qa
(let
    ((fake
      (car (read-from-string "#s(window-configuration nil nil nil)"))))
  (list (recordp fake) (window-configuration-p fake)
	(prin1-to-string fake)))
'(t nil "#s(window-configuration nil nil nil)")
)

(qa
(let ((fake (record 'window-configuration nil nil nil)))
  (list (equal fake (copy-sequence fake))
	(eq fake (copy-sequence fake))))
'(t nil)
)

(qa
(let
    ((real (current-window-configuration))
     (fake (record 'window-configuration nil nil nil)))
  (list (equal real fake) (equal fake real) (equal real real)))
'(nil nil t)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((before (current-window-configuration)))
     (goto-char 12) (set-window-start window 12 t)
     (set-window-hscroll window 3)
     (let ((after (current-window-configuration)))
       (list (equal before after) (equal (list before) (list after))
	     (equal-including-properties before after))))))
'(nil nil nil)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window)
   (let ((before (current-window-configuration)))
     (set-window-margins window 2 3)
     (let ((after (current-window-configuration)))
       (list (equal before after)
	     (equal (vector before) (vector after)))))))
'(nil nil)
)

(qa
(qa277-window-fixture
 (lambda (_buffer window) (require 'cl-lib)
   (let ((before (current-window-configuration)))
     (set-window-hscroll window 3)
     (cl-equalp before (current-window-configuration)))))
'nil
)

(qa
(let ((saved (current-window-configuration)))
  (dotimes (_ 20) (current-window-configuration)) (garbage-collect)
  (list (window-configuration-p saved) (recordp saved)
	(window-configuration-p
	 (record 'window-configuration (selected-frame) nil nil))))
'(t nil nil)
)

(qa
(qa280-probe 'real 'fillarray)
'(wrong-type-argument arrayp)
)

(qa
(let
    ((a (current-window-configuration))
     (b (current-window-configuration)))
  (list (eq a b) (equal a b) (equal a a) (equal (list a) (list b))
	(compare-window-configurations a b)))
'(nil nil t nil t)
)


;; Issue #284: snapshots own persistent associations and merge only saved keys.
(defun qa284-with-window (function)
 (qa277-window-fixture (lambda (_buffer old)
  (let ((new (split-window old nil 'right))) (funcall function new (frame-root-window))))))
(defun qa284-basic (flag internal)
 (qa284-with-window
  (lambda (leaf root)
   (let* ((window (if internal root leaf)) (key (make-symbol "qa284"))
          (window-persistent-parameters (list (cons key flag))))
    (set-window-parameter window key 1)
    (let ((saved (current-window-configuration)))
     (set-window-parameter window key 2) (set-window-configuration saved)
     (let ((first (window-parameter window key)))
      (set-window-parameter window key 3) (set-window-configuration saved)
      (list first (window-parameter window key))))))))
(qa
(qa284-basic nil nil)
'(2 3)
)

(qa
(qa284-basic nil t)
'(2 3)
)

(qa
(qa284-basic t nil)
'(1 1)
)

(qa
(qa284-basic t t)
'(1 1)
)

(qa
(qa284-basic 'writable nil)
'(1 1)
)

(qa
(qa284-basic 'writable t)
'(1 1)
)

(qa
(qa284-basic 'other nil)
'(1 1)
)

(qa
(qa284-basic 'other t)
'(1 1)
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-absent"))
	(window-persistent-parameters (list (cons key t)))
	(saved (current-window-configuration)))
     (set-window-configuration saved)
     (list (window-parameter window key)
	   (not (null (assq key (window-parameters window))))))))
'(nil nil)
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-absent"))
	(window-persistent-parameters (list (cons key t)))
	(saved (current-window-configuration)))
     (set-window-parameter window key 2)
     (set-window-configuration saved)
     (list (window-parameter window key)
	   (not (null (assq key (window-parameters window))))))))
'(nil t)
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-nil"))
	(window-persistent-parameters (list (cons key t))))
     (set-window-parameter window key nil)
     (let ((saved (current-window-configuration)))
       (set-window-parameter window key 2)
       (set-window-configuration saved)
       (list (window-parameter window key)
	     (not (null (assq key (window-parameters window)))))))))
'(nil t)
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-sharing")) (value (list 1))
	(window-persistent-parameters (list (cons key t))))
     (set-window-parameter window key value)
     (let ((saved (current-window-configuration)))
       (setcar value 9) (set-window-parameter window key 'other)
       (set-window-configuration saved)
       (list (eq (window-parameter window key) value)
	     (window-parameter window key))))))
'(t (9))
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-change-flags"))
	(window-persistent-parameters (list (cons key t))))
     (set-window-parameter window key 1)
     (let ((saved (current-window-configuration)))
       (let ((window-persistent-parameters nil))
	 (set-window-parameter window key 2)
	 (set-window-configuration saved))
       (window-parameter window key)))))
'1
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-other"))
	(keep (make-symbol "qa284-persistent"))
	(window-persistent-parameters (list (cons keep t))))
     (set-window-parameter window keep 1)
     (set-window-parameter window key 'old)
     (let ((saved (current-window-configuration)))
       (set-window-parameter window keep 2)
       (set-window-parameter window key 'new)
       (set-window-configuration saved)
       (list (window-parameter window keep)
	     (window-parameter window key))))))
'(1 new)
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-cycle")) (entry (list (cons key t)))
	(window-persistent-parameters entry))
     (setcdr entry entry) (set-window-parameter window key 1)
     (let ((saved (current-window-configuration)))
       (set-window-parameter window key 2)
       (set-window-configuration saved)
       (list (null window-persistent-parameters)
	     (window-parameter window key))))))
'(t 2)
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-dotted"))
	(window-persistent-parameters (cons (cons key t) 'tail)))
     (set-window-parameter window key 1)
     (let ((saved (current-window-configuration)))
       (set-window-parameter window key 2)
       (set-window-configuration saved) (window-parameter window key)))))
'1
)

(qa
(qa284-with-window
 (lambda (window _root)
   (let*
       ((key (make-symbol "qa284-junk"))
	(window-persistent-parameters (list 'junk nil 42 (cons key t))))
     (set-window-parameter window key 1)
     (let ((saved (current-window-configuration)))
       (set-window-parameter window key 2)
       (set-window-configuration saved) (window-parameter window key)))))
'1
)


(princ (qa--done)) (terpri)
