;;; -*- lexical-binding: t; -*-
;; Issue #263: OVERRIDES validates and dynamically binds print settings.
(defun qa263-observe (key variable value)
 (let ((before (symbol-value variable)) (seen nil) (calls 0))
  (prin1 'a (lambda (_character) (setq seen (symbol-value variable) calls (1+ calls)))
         (list (cons key value)))
  (list (> calls 0) (eq seen value) (eq (symbol-value variable) before))))
(defvar qa263-ordinary nil)
(qa
(prin1-to-string '(1 2 3) nil (list (cons 'length nil)))
'"(1 2 3)"
)

(qa
(prin1-to-string [1 2 3] nil (list (cons 'length nil)))
'"[1 2 3]"
)

(qa
(prin1-to-string '(1 2 3) nil (list (cons 'length 0)))
'"(...)"
)

(qa
(prin1-to-string [1 2 3] nil (list (cons 'length 0)))
'"[...]"
)

(qa
(prin1-to-string '(1 2 3) nil (list (cons 'length 1)))
'"(1 ...)"
)

(qa
(prin1-to-string [1 2 3] nil (list (cons 'length 1)))
'"[1 ...]"
)

(qa
(prin1-to-string '(1 2 3) nil (list (cons 'length 2)))
'"(1 2 ...)"
)

(qa
(prin1-to-string [1 2 3] nil (list (cons 'length 2)))
'"[1 2 ...]"
)

(qa
(prin1-to-string '(1 2 3) nil (list (cons 'length 4)))
'"(1 2 3)"
)

(qa
(prin1-to-string [1 2 3] nil (list (cons 'length 4)))
'"[1 2 3]"
)

(qa
(prin1-to-string '((a (b)) c) nil (list (cons 'level nil)))
'"((a (b)) c)"
)

(qa
(prin1-to-string '((a (b)) c) nil (list (cons 'level 0)))
'"..."
)

(qa
(prin1-to-string '((a (b)) c) nil (list (cons 'level 1)))
'"(... c)"
)

(qa
(prin1-to-string '((a (b)) c) nil (list (cons 'level 2)))
'"((a ...) c)"
)

(qa
(let ((print-length 1))
  (list (prin1-to-string '(1 2 3) nil t) print-length))
'("(1 2 3)" 1)
)

(qa
(let ((print-length 1))
  (list
   (prin1-to-string '(1 2 3 4) nil '((length . 2) t (length . 3)))
   print-length))
'("(1 2 3 ...)" 1)
)

(qa
(let ((print-length 1))
  (prin1-to-string '(1 2 3) nil '((length . 2) t)))
'"(1 2 3)"
)

(qa
(prin1-to-string '(1 2 3) nil '((length . 1) (length . 2)))
'"(1 2 ...)"
)

(qa
(prin1-to-string ''a nil '((quoted)))
'"(quote a)"
)

(qa
(prin1-to-string ''a nil '((quoted . t)))
'"'a"
)

(qa
(prin1-to-string 1.23456 nil '((float-format . "%.2f")))
'"1.23"
)

(qa
(prin1-to-string 65 nil '((integers-as-characters . t)))
'"?A"
)

(qa
(prin1-to-string "a\nb" nil '((escape-newlines . t)))
'"\"a\\nb\""
)

(qa
(prin1-to-string "a	b" nil '((escape-control-characters . t)))
'"\"a\\11b\""
)

(qa
(prin1-to-string "é" nil '((escape-nonascii . t)))
'"\"é\""
)

(qa
(prin1-to-string "é" nil '((escape-multibyte . t)))
'"\"\\x00e9\""
)

(qa
(prin1-to-string "a\nb" t '((escape-newlines . t)))
'"a\nb"
)

(qa
(prin1-to-string (make-symbol "qa263") nil '((gensym . t)))
'"#:qa263"
)

(qa
(let ((x (list 1 2)))
  (prin1-to-string (list x x) nil '((circle . t))))
'"(#1=(1 2) #1#)"
)

(qa
(with-temp-buffer
  (prin1 '(1 2 3) (current-buffer) '((length . 1))) (buffer-string))
'"(1 ...)"
)

(qa
(with-temp-buffer
  (let ((marker (point-marker)))
    (prin1 '(1 2 3) marker '((length . 1)))
    (list (buffer-string) (marker-position marker))))
'("(1 ...)" 8)
)

(qa
(let ((print-length 3) (seen nil))
  (list
   (condition-case err
       (prin1 '(1 2 3)
	      (lambda (_character) (setq seen print-length)
		(error "sink failed"))
	      '((length . 1)))
     (error (list (car err) (cdr err))))
   seen print-length))
'((error ("sink failed")) 1 3)
)

(qa
(let ((print-length 3) (seen nil))
  (list
   (prin1-to-string (current-buffer) nil
		    (list '(length . 1)
			  (cons 'unreadable-function
				(lambda (_object _escape)
				  (setq seen
					(list print-length
					      inhibit-modification-hooks))
				  "opaque"))))
   seen print-length))
'("opaque" (1 t) 3)
)

(qa
(let ((print-length 3) (seen nil))
  (prin1-to-string (current-buffer) nil
		   (list '(length . 1)
			 (cons 'unreadeable-function
			       (lambda (_object _escape)
				 (setq seen
				       (list print-length
					     (prin1-to-string '(1 2 3)
							      nil
							      '((length
								 . 2)))
					     print-length))
				 "opaque"))))
  (list seen print-length))
'((1 "(1 2 ...)" 1) 3)
)

(qa
(qa263-observe 'length 'print-length 1)
'(t t t)
)

(qa
(qa263-observe 'level 'print-level 1)
'(t t t)
)

(qa
(qa263-observe 'circle 'print-circle t)
'(t t t)
)

(qa
(qa263-observe 'quoted 'print-quoted nil)
'(t t t)
)

(qa
(qa263-observe 'escape-newlines 'print-escape-newlines t)
'(t t t)
)

(qa
(qa263-observe 'escape-control-characters
	       'print-escape-control-characters t)
'(t t t)
)

(qa
(qa263-observe 'escape-nonascii 'print-escape-nonascii t)
'(t t t)
)

(qa
(qa263-observe 'escape-multibyte 'print-escape-multibyte t)
'(t t t)
)

(qa
(qa263-observe 'charset-text-property 'print-charset-text-property t)
'(t t t)
)

(qa
(qa263-observe 'unreadable-function 'print-unreadable-function
	       'qa263-unused)
'(t t t)
)

(qa
(qa263-observe 'unreadeable-function 'print-unreadable-function
	       'qa263-unused)
'(t t t)
)

(qa
(qa263-observe 'gensym 'print-gensym t)
'(t t t)
)

(qa
(qa263-observe 'continuous-numbering 'print-continuous-numbering t)
'(t t t)
)

(qa
(qa263-observe 'number-table 'print-number-table
	       (make-hash-table :test 'eq))
'(t nil t)
)

(qa
(qa263-observe 'float-format 'float-output-format "%.2f")
'(t t t)
)

(qa
(qa263-observe 'integers-as-characters 'print-integers-as-characters t)
'(t t t)
)

(qa
(qa263-observe 'symbols-bare 'print-symbols-bare t)
'(t t t)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err (prin1-to-string '(1 2 3) nil 42) (error err))
   print-length))
'((wrong-type-argument consp 42) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err (prin1-to-string '(1 2 3) nil 'bad)
     (error err))
   print-length))
'((wrong-type-argument consp bad) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err (prin1-to-string '(1 2 3) nil []) (error err))
   print-length))
'((wrong-type-argument consp []) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err (prin1-to-string '(1 2 3) nil '(length))
     (error err))
   print-length))
'((wrong-type-argument consp length) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err (prin1-to-string '(1 2 3) nil '(nil))
     (error err))
   print-length))
'((wrong-type-argument consp nil) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err
       (prin1-to-string '(1 2 3) nil '((length . 1) bad))
     (error err))
   print-length))
'((wrong-type-argument consp bad) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err
       (prin1-to-string '(1 2 3) nil '((length . 1) . bad))
     (error err))
   print-length))
'((wrong-type-argument consp ((length . 1) . bad)) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err
       (prin1-to-string '(1 2 3) nil '((not-a-print-setting . 1)))
     (error err))
   print-length))
'((wrong-type-argument symbolp nil) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err
       (prin1-to-string '(1 2 3) nil '((print-length . 1)))
     (error err))
   print-length))
'((wrong-type-argument symbolp nil) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err (prin1-to-string '(1 2 3) nil '((nil . 1)))
     (error err))
   print-length))
'((wrong-type-argument symbolp nil) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err
       (prin1-to-string '(1 2 3) nil '(("length" . 1)))
     (error err))
   print-length))
'((wrong-type-argument symbolp nil) 3)
)

(qa
(let ((print-length 3))
  (list
   (condition-case err
       (prin1-to-string '(1 2 3) nil
			(list (cons (make-symbol "length") 1)))
     (error err))
   print-length))
'((wrong-type-argument symbolp nil) 3)
)

(qa
(condition-case err (prin1 '(1) 42 '((unknown . 1))) (error err))
'(wrong-type-argument symbolp nil)
)

(qa
(let
    ((print-length 2) (print-level 3) (print-quoted nil)
     (print-circle t) (print-gensym t) (float-output-format "%.2f")
     (seen nil))
  (prin1 'a
	 (lambda (_character)
	   (setq seen
		 (list print-length print-level print-quoted
		       print-circle print-gensym float-output-format)))
	 t)
  (list seen print-length print-level print-quoted print-circle
	print-gensym float-output-format))
'((nil nil t nil nil nil) 2 3 nil t t "%.2f")
)

(qa
(let ((print-length 3) (events nil))
  (let
      ((watcher
	(lambda (_symbol value operation _where)
	  (push (list operation value print-length) events))))
    (unwind-protect
	(progn
	  (add-variable-watcher 'print-length watcher)
	  (let
	      ((text
		(prin1-to-string '(1 2 3) nil
				 '((length . 1) (length . 2)))))
	    (list text print-length (reverse events))))
      (remove-variable-watcher 'print-length watcher))))
'("(1 2 ...)" 3
  ((let 1 3) (let 2 1) (set 1 2) (unlet 1 2) (set 3 1) (unlet 3 1)))
)

(qa
(let ((print-length 3) (events nil))
  (let
      ((watcher
	(lambda (_symbol value operation _where)
	  (push (list operation value) events))))
    (unwind-protect
	(progn
	  (add-variable-watcher 'print-length watcher)
	  (list
	   (condition-case err
	       (prin1-to-string '(1 2 3) nil '((length . 1) (bad . 2)))
	     (error err))
	   print-length (reverse events)))
      (remove-variable-watcher 'print-length watcher))))
'((wrong-type-argument symbolp nil) 3 ((let 1) (set 3) (unlet 3)))
)

(qa
(let
    ((first (generate-new-buffer " *qa263-first*"))
     (second (generate-new-buffer " *qa263-second*")))
  (unwind-protect
      (let ((standard-output first) (print-length 3))
	(let
	    ((watcher
	      (lambda (_symbol _value operation _where)
		(when (eq operation 'let)
		  (setq standard-output second)))))
	  (unwind-protect
	      (progn
		(add-variable-watcher 'print-length watcher)
		(prin1 '(1 2 3) nil '((length . 1)))
		(list (with-current-buffer first (buffer-string))
		      (with-current-buffer second (buffer-string))
		      print-length))
	    (remove-variable-watcher 'print-length watcher))))
    (kill-buffer first) (kill-buffer second)))
'("(1 ...)" "" 3)
)

(qa
(let ((max-specpdl-size 50))
  (prin1-to-string '(1 2 3) nil (make-list 200 '(length . 1))))
'"(1 ...)"
)

(qa
(let ((seen nil) (print-unreadable-function nil))
  (prin1-to-string (current-buffer) nil
		   (list
		    (cons 'unreadable-function
			  (lambda (_object _escape)
			    (setq seen
				  (null print-unreadable-function))
			    "opaque"))))
  (list seen print-unreadable-function))
'(t nil)
)

(qa
(let ((seen nil) (calls 0) (print-unreadable-function nil))
  (prin1-to-string (current-buffer) nil
		   (list
		    (cons 'unreadable-function
			  (lambda (object _escape)
			    (setq calls (1+ calls) seen
				  (string-match-p "^#<buffer"
						  (prin1-to-string
						   object)))
			    "opaque"))))
  (list calls seen print-unreadable-function))
'(1 0 nil)
)

(qa
(let ((qa263-ordinary 3) (events nil))
  (let
      ((watcher
	(lambda (_symbol value operation _where)
	  (push (list operation value qa263-ordinary) events))))
    (unwind-protect
	(progn
	  (add-variable-watcher 'qa263-ordinary watcher)
	  (let ((qa263-ordinary 1)) nil)
	  (list qa263-ordinary (reverse events)))
      (remove-variable-watcher 'qa263-ordinary watcher))))
'(3 ((let 1 3) (unlet 3 1)))
)

(qa
(let ((print-length 3))
  (let
      ((watcher
	(lambda (_symbol _value operation _where)
	  (when (eq operation 'let) (setq print-length 99)))))
    (unwind-protect
	(progn
	  (add-variable-watcher 'print-length watcher)
	  (list (prin1-to-string '(1 2 3) nil '((length . 1)))
		print-length))
      (remove-variable-watcher 'print-length watcher))))
'("(1 ...)" 3)
)

(qa
(let ((print-length 3) (events nil))
  (let
      ((watcher
	(lambda (_symbol value operation _where)
	  (push (list operation value print-length) events)
	  (when (eq operation 'let) (error "binding failed")))))
    (unwind-protect
	(progn
	  (add-variable-watcher 'print-length watcher)
	  (list
	   (condition-case err
	       (prin1-to-string '(1 2 3) nil '((length . 1)))
	     (error (list (car err) (cdr err))))
	   print-length (reverse events)))
      (remove-variable-watcher 'print-length watcher))))
'((error ("binding failed")) 3 ((let 1 3) (set 3 3) (unlet 3 3)))
)


(princ (qa--done)) (terpri)
