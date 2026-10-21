;;; -*- lexical-binding: t; -*-
;; Issue #246: all execution paths use GNU mark save/restore cleanup.
(defvar qa246-events nil)
(defvar qa246-inner nil)
(defvar qa246-inner-active nil)
(defun qa246-body ()
  (set-marker (mark-marker) qa246-inner (current-buffer))
  (setq mark-active qa246-inner-active)
  (goto-char 4)
  'done)
(defun qa246-state () (list (point) (mark t) mark-active))
(defun qa246-run (mode initial active inner inner-active)
  (with-temp-buffer
    (insert "abcd") (goto-char 2)
    (set-marker (mark-marker) initial (current-buffer)) (setq mark-active active)
    (let ((qa246-events nil) (qa246-inner inner) (qa246-inner-active inner-active)
          (activate-mark-hook (list (lambda () (push (cons 'activate (qa246-state)) qa246-events))))
          (deactivate-mark-hook (list (lambda () (push (cons 'deactivate (qa246-state)) qa246-events)))))
      (let ((result
             (cond ((eq mode 'literal) (save-mark-and-excursion (qa246-body)))
                   ((eq mode 'expanded) (eval (macroexpand '(save-mark-and-excursion (qa246-body))) t))
                   ((eq mode 'compiled) (funcall (byte-compile '(lambda () (save-mark-and-excursion (qa246-body)))))))))
        (list result (qa246-state) (reverse qa246-events))))))

;; Use lexical eval for the same binding model as the GNU oracle.
;; Hook-count fixtures bind transient-mark-mode explicitly; the library
;; harness and CLI startup have different preexisting mode defaults.
(qa-lex
 (with-temp-buffer
   (insert "abc") (goto-char 1) (set-marker (mark-marker) nil)
   (save-mark-and-excursion
     (set-mark 2) (setq mark-active t) (goto-char 3))
   (list (point) (mark t) mark-active))
 '(1 nil nil))

(qa-lex
 (let ((transient-mark-mode nil))
   (with-temp-buffer
     (insert "abc") (set-mark 2) (setq mark-active t)
     (let ((hits 0))
       (let
	   ((activate-mark-hook
	     (list (lambda nil (setq hits (1+ hits))))))
	 (save-mark-and-excursion (set-mark 3) (setq mark-active nil)))
       hits)))
 '2)

(qa-lex (qa246-run 'literal nil nil nil nil) '(done (2 nil nil) nil))

(qa-lex (qa246-run 'literal nil nil nil t)
	'(done (2 nil nil) ((deactivate 2 nil nil))))

(qa-lex (qa246-run 'literal nil nil 3 nil) '(done (2 nil nil) nil))

(qa-lex (qa246-run 'literal nil nil 3 t)
	'(done (2 nil nil) ((deactivate 2 nil nil))))

(qa-lex (qa246-run 'literal nil t nil nil)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'literal nil t nil t) '(done (2 nil t) nil))

(qa-lex (qa246-run 'literal nil t 3 nil)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'literal nil t 3 t)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'literal nil 'tag nil nil)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'literal nil 'tag nil t) '(done (2 nil tag) nil))

(qa-lex (qa246-run 'literal nil 'tag 3 nil)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'literal nil 'tag 3 t)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'literal 2 nil nil nil) '(done (2 2 nil) nil))

(qa-lex (qa246-run 'literal 2 nil nil t)
	'(done (2 2 nil) ((deactivate 2 2 nil))))

(qa-lex (qa246-run 'literal 2 nil 3 nil) '(done (2 2 nil) nil))

(qa-lex (qa246-run 'literal 2 nil 3 t)
	'(done (2 2 nil) ((deactivate 2 2 nil))))

(qa-lex (qa246-run 'literal 2 t nil nil)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'literal 2 t nil t)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'literal 2 t 3 nil)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'literal 2 t 3 t)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'literal 2 'tag nil nil)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'literal 2 'tag nil t)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'literal 2 'tag 3 nil)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'literal 2 'tag 3 t)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'expanded nil nil nil nil) '(done (2 nil nil) nil))

(qa-lex (qa246-run 'expanded nil nil nil t)
	'(done (2 nil nil) ((deactivate 2 nil nil))))

(qa-lex (qa246-run 'expanded nil nil 3 nil) '(done (2 nil nil) nil))

(qa-lex (qa246-run 'expanded nil nil 3 t)
	'(done (2 nil nil) ((deactivate 2 nil nil))))

(qa-lex (qa246-run 'expanded nil t nil nil)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'expanded nil t nil t) '(done (2 nil t) nil))

(qa-lex (qa246-run 'expanded nil t 3 nil)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'expanded nil t 3 t)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'expanded nil 'tag nil nil)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'expanded nil 'tag nil t) '(done (2 nil tag) nil))

(qa-lex (qa246-run 'expanded nil 'tag 3 nil)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'expanded nil 'tag 3 t)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'expanded 2 nil nil nil) '(done (2 2 nil) nil))

(qa-lex (qa246-run 'expanded 2 nil nil t)
	'(done (2 2 nil) ((deactivate 2 2 nil))))

(qa-lex (qa246-run 'expanded 2 nil 3 nil) '(done (2 2 nil) nil))

(qa-lex (qa246-run 'expanded 2 nil 3 t)
	'(done (2 2 nil) ((deactivate 2 2 nil))))

(qa-lex (qa246-run 'expanded 2 t nil nil)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'expanded 2 t nil t)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'expanded 2 t 3 nil)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'expanded 2 t 3 t)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'expanded 2 'tag nil nil)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'expanded 2 'tag nil t)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'expanded 2 'tag 3 nil)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'expanded 2 'tag 3 t)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'compiled nil nil nil nil) '(done (2 nil nil) nil))

(qa-lex (qa246-run 'compiled nil nil nil t)
	'(done (2 nil nil) ((deactivate 2 nil nil))))

(qa-lex (qa246-run 'compiled nil nil 3 nil) '(done (2 nil nil) nil))

(qa-lex (qa246-run 'compiled nil nil 3 t)
	'(done (2 nil nil) ((deactivate 2 nil nil))))

(qa-lex (qa246-run 'compiled nil t nil nil)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'compiled nil t nil t) '(done (2 nil t) nil))

(qa-lex (qa246-run 'compiled nil t 3 nil)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'compiled nil t 3 t)
	'(done (2 nil t) ((activate 2 nil t))))

(qa-lex (qa246-run 'compiled nil 'tag nil nil)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'compiled nil 'tag nil t) '(done (2 nil tag) nil))

(qa-lex (qa246-run 'compiled nil 'tag 3 nil)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'compiled nil 'tag 3 t)
	'(done (2 nil tag) ((activate 2 nil tag))))

(qa-lex (qa246-run 'compiled 2 nil nil nil) '(done (2 2 nil) nil))

(qa-lex (qa246-run 'compiled 2 nil nil t)
	'(done (2 2 nil) ((deactivate 2 2 nil))))

(qa-lex (qa246-run 'compiled 2 nil 3 nil) '(done (2 2 nil) nil))

(qa-lex (qa246-run 'compiled 2 nil 3 t)
	'(done (2 2 nil) ((deactivate 2 2 nil))))

(qa-lex (qa246-run 'compiled 2 t nil nil)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'compiled 2 t nil t)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'compiled 2 t 3 nil)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'compiled 2 t 3 t)
	'(done (2 2 t) ((activate 2 2 t))))

(qa-lex (qa246-run 'compiled 2 'tag nil nil)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'compiled 2 'tag nil t)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'compiled 2 'tag 3 nil)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex (qa246-run 'compiled 2 'tag 3 t)
	'(done (2 2 tag) ((activate 2 2 tag))))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2)
   (let ((saved-marker 'outer))
     (save-mark-and-excursion (setq saved-marker 'body) (goto-char 4))
     (list saved-marker (point))))
 '(body 2))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2)
   (let ((saved-marker 'outer))
     (eval
      (macroexpand
       '(save-mark-and-excursion (setq saved-marker 'body)
				 (goto-char 4)))
      t)
     (list saved-marker (point))))
 '(outer 2))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2)
   (let ((saved-marker 'outer))
     (funcall
      (byte-compile
       '(lambda nil
	  (save-mark-and-excursion
	    (setq saved-marker 'body) (goto-char 4)))))
     (list saved-marker (point))))
 '(outer 2))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active 'tag)
   (let ((saved (save-mark-and-excursion--save)))
     (list (marker-position (car saved))
	   (marker-insertion-type (car saved)) (cdr saved))))
 '(3 nil tag))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-marker (mark-marker) nil)
   (setq mark-active 'tag) (save-mark-and-excursion--save))
 '(nil . tag))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active nil)
   (let ((mark-active 'dynamic))
     (save-mark-and-excursion
       (setq mark-active nil) (set-marker (mark-marker) 1))
     (qa246-state)))
 '(2 3 dynamic))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active t)
   (let
       ((qa246-events nil)
	(activate-mark-hook
	 (list (lambda nil (push (qa246-state) qa246-events)))))
     (list
      (condition-case err
	  (save-mark-and-excursion
	    (set-marker (mark-marker) 1) (setq mark-active nil)
	    (goto-char 4) (error "body"))
	(error (error-message-string err)))
      (qa246-state) qa246-events)))
 '("body" (2 3 t) ((2 3 t))))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active t)
   (let
       ((qa246-events nil)
	(activate-mark-hook
	 (list (lambda nil (push (qa246-state) qa246-events)))))
     (list
      (catch 'qa246-exit
	(save-mark-and-excursion
	  (set-marker (mark-marker) 1) (setq mark-active nil)
	  (goto-char 4) (throw 'qa246-exit 'thrown)))
      (qa246-state) qa246-events)))
 '(thrown (2 3 t) ((2 3 t))))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active t)
   (let ((activate-mark-hook (list (lambda nil (error "cleanup")))))
     (list
      (condition-case err
	  (save-mark-and-excursion
	    (set-marker (mark-marker) 1) (setq mark-active nil)
	    (goto-char 4) (error "body"))
	(error (error-message-string err)))
      (qa246-state))))
 '("cleanup" (2 3 t)))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active t)
   (save-mark-and-excursion
     (goto-char 1) (insert "XX") (set-marker (mark-marker) 1)
     (setq mark-active nil) (garbage-collect))
   (list (buffer-string) (qa246-state)))
 '("XXabcd" (4 5 t)))

(qa-lex
 (let ((other (generate-new-buffer " *qa246-other*")))
   (unwind-protect
       (with-temp-buffer
	 (insert "abcd") (goto-char 2) (set-mark 3)
	 (setq mark-active t)
	 (let ((original (current-buffer)))
	   (save-mark-and-excursion
	     (set-marker (mark-marker) 1) (setq mark-active nil)
	     (set-buffer other))
	   (list (eq original (current-buffer)) (qa246-state))))
     (kill-buffer other)))
 '(t (2 3 t)))

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active t)
   (let
       ((qa246-events nil)
	(activate-mark-hook
	 (list (lambda nil (push (qa246-state) qa246-events)))))
     (save-mark-and-excursion
       (set-marker (mark-marker) 1) (setq mark-active nil)
       (goto-char 4)
       (save-mark-and-excursion
	 (set-marker (mark-marker) 2) (setq mark-active t)
	 (goto-char 1)))
     (list (qa246-state) (reverse qa246-events))))
 '((2 3 t) ((2 3 t))))

(qa-lex
 (let ((other (generate-new-buffer " *qa246-foreign*")))
   (unwind-protect
       (progn
	 (with-current-buffer other (insert "wxyz"))
	 (with-temp-buffer
	   (insert "abcd") (goto-char 2)
	   (set-marker (mark-marker) 3 other) (setq mark-active t)
	   (save-mark-and-excursion
	     (with-current-buffer other (goto-char 1) (insert "X"))
	     (set-marker (mark-marker) 1 (current-buffer)))
	   (list (qa246-state)
		 (eq (marker-buffer (mark-marker)) (current-buffer)))))
     (kill-buffer other)))
 '((2 4 t) t))

(qa-lex
 (let ((transient-mark-mode t))
   (with-temp-buffer
     (insert "abc") (set-mark 2) (setq mark-active t)
     (let ((hits 0))
       (let
	   ((activate-mark-hook
	     (list (lambda nil (setq hits (1+ hits))))))
	 (save-mark-and-excursion (set-mark 3) (setq mark-active nil)))
       hits)))
 '1)

(qa-lex
 (with-temp-buffer
   (insert "abcd") (goto-char 2) (set-mark 3) (setq mark-active t)
   (let
       ((qa246-events nil)
	(activate-mark-hook
	 (list (lambda nil (push (qa246-state) qa246-events)))))
     (list
      (condition-case nil
	  (save-mark-and-excursion
	    (goto-char 4) (set-marker (mark-marker) 1)
	    (setq mark-active nil) (signal 'quit nil))
	(quit 'caught))
      (qa246-state) qa246-events)))
 '(caught (2 3 t) ((2 3 t))))


(princ (qa--done)) (terpri)
