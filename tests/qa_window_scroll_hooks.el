;;; -*- lexical-binding: t; -*-
(defun qa283-with-windows (function)
 (let ((saved (current-window-configuration))
       (caller (generate-new-buffer " *qa283-caller*"))
       (target (generate-new-buffer " *qa283-target*")))
  (unwind-protect
   (progn
    (delete-other-windows) (switch-to-buffer caller) (insert "caller\nline\n") (goto-char 1)
    (with-current-buffer target (insert "other\nbuffer\n"))
    (let* ((selected (selected-window)) (other (split-window selected nil 'right)))
     (set-window-buffer other target) (set-window-start other 5 t)
     (funcall function caller target selected other)))
   (set-window-configuration saved)
   (when (buffer-live-p caller) (kill-buffer caller))
   (when (buffer-live-p target) (kill-buffer target)))))
(defun qa283-run (argument action)
 (qa283-with-windows
  (lambda (caller target selected other)
   (let* ((window (pcase argument ('selected selected) ('other other)))
          (buffer (if (eq argument 'other) target caller)) hits
          (window-scroll-functions
           (list (lambda (w start)
            (push (list (eq w window) (null w) start (eq (current-buffer) buffer)
                        (eq (selected-window) selected)) hits)
            (pcase action
             ('error (error "scroll hook"))
             ('throw (throw 'qa283 'jump))
             ('point (goto-char 3))
             ('switch (set-buffer caller))
             ('select (select-window other)))
            'ignored))))
    (let ((result (catch 'qa283
                   (condition-case e
                    (if (eq argument 'omitted) (run-window-scroll-functions)
                     (run-window-scroll-functions window))
                    (error e)))))
     (list result (nreverse hits) (eq (current-buffer) caller)
      (eq (selected-window) selected) (with-current-buffer buffer (point))))))))

(require 'cl-lib)
(qa
(qa283-run 'omitted 'normal)
'(nil ((t t 1 t t)) t t 1)
)

(qa
(qa283-run 'nil 'normal)
'(nil ((t t 1 t t)) t t 1)
)

(qa
(qa283-run 'selected 'normal)
'(nil ((t nil 1 t t)) t t 1)
)

(qa
(qa283-run 'other 'normal)
'(nil ((t nil 5 t t)) t t 14)
)

(qa
(qa283-run 'other 'error)
'((error "scroll hook") ((t nil 5 t t)) t t 14)
)

(qa
(qa283-run 'other 'throw)
'(jump ((t nil 5 t t)) t t 14)
)

(qa
(qa283-run 'other 'point)
'(nil ((t nil 5 t t)) t t 3)
)

(qa
(qa283-run 'other 'switch)
'(nil ((t nil 5 t t)) t t 14)
)

(qa
(qa283-run 'other 'select)
'(nil ((t nil 5 t t)) t nil 14)
)

(qa
(qa283-with-windows
 (lambda (caller target selected other)
   (let (hits)
     (cl-labels
	 ((global-hook (w start)
	    (push
	     (list 'global (eq w other) start
		   (eq (current-buffer) target))
	     hits))
	  (target-hook (w start)
	    (push
	     (list 'local (eq w other) start
		   (eq (current-buffer) target))
	     hits)))
       (let ((old (default-value 'window-scroll-functions)))
	 (unwind-protect
	     (progn
	       (set-default 'window-scroll-functions
			    (list #'global-hook))
	       (with-current-buffer target
		 (setq-local window-scroll-functions nil))
	       (run-window-scroll-functions other) (nreverse hits))
	   (set-default 'window-scroll-functions old)))))))
'nil
)

(qa
(qa283-with-windows
 (lambda (caller target selected other)
   (let (hits)
     (cl-labels
	 ((global-hook (w start)
	    (push
	     (list 'global (eq w other) start
		   (eq (current-buffer) target))
	     hits))
	  (target-hook (w start)
	    (push
	     (list 'local (eq w other) start
		   (eq (current-buffer) target))
	     hits)))
       (let ((old (default-value 'window-scroll-functions)))
	 (unwind-protect
	     (progn
	       (set-default 'window-scroll-functions
			    (list #'global-hook))
	       (with-current-buffer target
		 (setq-local window-scroll-functions
			     (list #'target-hook)))
	       (run-window-scroll-functions other) (nreverse hits))
	   (set-default 'window-scroll-functions old)))))))
'((local t 5 t))
)

(qa
(qa283-with-windows
 (lambda (caller target selected other)
   (let (hits)
     (cl-labels
	 ((global-hook (w start)
	    (push
	     (list 'global (eq w other) start
		   (eq (current-buffer) target))
	     hits))
	  (target-hook (w start)
	    (push
	     (list 'local (eq w other) start
		   (eq (current-buffer) target))
	     hits)))
       (let ((old (default-value 'window-scroll-functions)))
	 (unwind-protect
	     (progn
	       (set-default 'window-scroll-functions
			    (list #'global-hook))
	       (with-current-buffer target
		 (setq-local window-scroll-functions
			     (list #'target-hook t)))
	       (run-window-scroll-functions other) (nreverse hits))
	   (set-default 'window-scroll-functions old)))))))
'((local t 5 t) (global t 5 t))
)

(qa
(qa283-with-windows
 (lambda (caller target selected other)
   (let (hits)
     (cl-labels
	 ((global-hook (w start)
	    (push
	     (list 'global (eq w other) start
		   (eq (current-buffer) target))
	     hits))
	  (target-hook (w start)
	    (push
	     (list 'local (eq w other) start
		   (eq (current-buffer) target))
	     hits)))
       (let ((old (default-value 'window-scroll-functions)))
	 (unwind-protect
	     (progn
	       (set-default 'window-scroll-functions
			    (list #'global-hook))
	       (with-current-buffer target
		 (setq-local window-scroll-functions
			     (list t #'target-hook)))
	       (run-window-scroll-functions other) (nreverse hits))
	   (set-default 'window-scroll-functions old)))))))
'((global t 5 t) (local t 5 t))
)

(qa
(save-window-excursion
  (let ((buffer (generate-new-buffer " *qa283-split*")))
    (unwind-protect
	(progn
	  (delete-other-windows) (switch-to-buffer buffer)
	  (insert "one\ntwo\n") (goto-char 1)
	  (let*
	      ((old (selected-window)) hits
	       (window-scroll-functions
		(list
		 (lambda (w start)
		   (push
		    (list w start (eq (current-buffer) buffer)
			  (window-live-p w)
			  (not (null (memq w (window-list)))))
		    hits)))))
	    (let ((new (split-window old nil 'right)))
	      (list (length hits) (eq (caar hits) new)
		    (cdr (car hits)) (eq (selected-window) old)))))
      (kill-buffer buffer))))
'(2 t (1 t t t) t)
)

(qa
(save-window-excursion
  (let ((buffer (generate-new-buffer " *qa283-split*")))
    (unwind-protect
	(progn
	  (delete-other-windows) (switch-to-buffer buffer)
	  (insert "one\ntwo\n") (goto-char 1)
	  (let*
	      ((old (selected-window)) hits
	       (window-scroll-functions
		(list
		 (lambda (w start)
		   (push
		    (list w start (eq (current-buffer) buffer)
			  (window-live-p w)
			  (not (null (memq w (window-list)))))
		    hits)))))
	    (let ((new (split-window old nil 'left)))
	      (list (length hits) (eq (caar hits) new)
		    (cdr (car hits)) (eq (selected-window) old)))))
      (kill-buffer buffer))))
'(2 t (1 t t t) t)
)

(qa
(save-window-excursion
  (let ((buffer (generate-new-buffer " *qa283-split*")))
    (unwind-protect
	(progn
	  (delete-other-windows) (switch-to-buffer buffer)
	  (insert "one\ntwo\n") (goto-char 1)
	  (let*
	      ((old (selected-window)) hits
	       (window-scroll-functions
		(list
		 (lambda (w start)
		   (push
		    (list w start (eq (current-buffer) buffer)
			  (window-live-p w)
			  (not (null (memq w (window-list)))))
		    hits)))))
	    (let ((new (split-window old nil 'above)))
	      (list (length hits) (eq (caar hits) new)
		    (cdr (car hits)) (eq (selected-window) old)))))
      (kill-buffer buffer))))
'(2 t (1 t t t) t)
)

(qa
(save-window-excursion
  (let ((buffer (generate-new-buffer " *qa283-split*")))
    (unwind-protect
	(progn
	  (delete-other-windows) (switch-to-buffer buffer)
	  (insert "one\ntwo\n") (goto-char 1)
	  (let*
	      ((old (selected-window)) hits
	       (window-scroll-functions
		(list
		 (lambda (w start)
		   (push
		    (list w start (eq (current-buffer) buffer)
			  (window-live-p w)
			  (not (null (memq w (window-list)))))
		    hits)))))
	    (let ((new (split-window old nil 'below)))
	      (list (length hits) (eq (caar hits) new)
		    (cdr (car hits)) (eq (selected-window) old)))))
      (kill-buffer buffer))))
'(2 t (1 t t t) t)
)

(qa
(qa283-with-windows
 (lambda (caller target selected other)
   (let ((saved (symbol-function 'run-hook-with-args)) hits)
     (unwind-protect
	 (let
	     ((window-scroll-functions
	       (list (lambda (&rest _) (push 'called hits)))))
	   (fset 'run-hook-with-args
		 (lambda (&rest _) (error "shadowed runner")))
	   (run-window-scroll-functions other) hits)
       (fset 'run-hook-with-args saved)))))
'(called)
)

(qa
(qa283-with-windows
 (lambda (caller target selected other)
   (let ((saved (symbol-function 'run-window-scroll-functions)) seen)
     (unwind-protect
	 (progn
	   (fset 'run-window-scroll-functions
		 (lambda (w) (setq seen w) 'ignored))
	   (let ((new (split-window selected nil 'below)))
	     (eq new seen)))
       (fset 'run-window-scroll-functions saved)))))
't
)

(qa
(condition-case e (run-window-scroll-functions 1)
  (error (list (car e) (cadr e))))
'(wrong-type-argument window-live-p)
)

(qa
(condition-case e (run-window-scroll-functions 'bad)
  (error (list (car e) (cadr e))))
'(wrong-type-argument window-live-p)
)


(princ (qa--done)) (terpri)
