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
(let ((print-length 1))
  (prin1-to-string (record 'qa-print-length-record 'a 'b)))
 '"#s(qa-print-length-record ...)"
)

(qa
(let ((print-length 1))
  (prin1-to-string (eval '(function (lambda (x) x)) nil)))
 '"#[(x) ...]"
)

;; Function-stream output of the opening bracket precedes print-length lookup.
(qa
(let* ((object [a b]) (characters nil) (count 0) (print-length nil))
  (prin1 object
         (lambda (character)
           (push character characters)
           (when (= (setq count (1+ count)) 1)
             (setq print-length 1))))
  (apply #'string (nreverse characters)))
 '"[a ...]"
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

;; GNU print.c initializes its C-backed number table without invoking
;; variable watchers, even when the Lisp binding is nil.
(defvar qa263-print-number-table-events nil)
(defun qa263-print-number-table-watcher (_symbol _value operation _where)
  (push operation qa263-print-number-table-events)
  (when (eq operation 'set)
    (signal 'file-error '("print-number-table watcher"))))
(qa
 (let ((print-circle t)
       (print-number-table nil)
       (qa263-print-number-table-events nil))
   (add-variable-watcher 'print-number-table
                         #'qa263-print-number-table-watcher)
   (unwind-protect
       (list (prin1-to-string '(a b))
             (hash-table-p print-number-table)
             qa263-print-number-table-events)
     (remove-variable-watcher 'print-number-table
                              #'qa263-print-number-table-watcher)))
 '("(a b)" t nil))

(qa
 (let ((print-circle t)
       (print-continuous-numbering t)
       (print-number-table nil))
   (let* ((cell (list 'a))
          (first (prin1-to-string (list cell cell)))
          (second (prin1-to-string (list cell cell))))
     (list first second (gethash cell print-number-table))))
 '("(#1=(a) #1#)" "(#1# #1#)" 1))

;; A nested print with circle disabled clears the shared number table. GNU
;; signals when the outer circle traversal next reaches a shared candidate.
(qa
 (let* ((cell (list 'a))
        (print-circle t)
        (print-continuous-numbering nil)
        (print-unreadable-function
         (lambda (_object _escape)
           (let ((print-circle nil))
             (prin1-to-string cell))
           "opaque")))
   (prin1-to-string (vector (current-buffer) cell cell)))
 '(qa--err wrong-type-argument (hash-table-p nil)))

(qa
 (let* ((cell (list 'a))
        (characters nil)
        (count 0)
        (print-circle t))
   (prin1 (vector cell cell)
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 1)
              (setq print-circle nil))))
   (apply #'string (nreverse characters)))
 "[(a) (a)]")

(qa
 (let* ((cell (list 'a))
        (print-circle t)
        (print-continuous-numbering nil)
        (print-unreadable-function
         (lambda (_object _escape)
           (let ((print-continuous-numbering t)
                 (print-number-table (make-hash-table :test 'eq)))
             (prin1-to-string cell))
           "opaque")))
   (prin1-to-string (vector (current-buffer) cell cell)))
 "[a)opaque #1=(a) #1#]")

;; A present key with a nil value is still unnumbered: GNU's gethash
;; preprocessing treats it like a missing entry and traverses its children.
(qa
 (progn
   (let ((print-circle t)) (print--preprocess nil))
   (let* ((cell (list 'a))
          (print-circle t)
          (print-continuous-numbering t)
          (print-number-table (make-hash-table :test 'eq)))
     (puthash cell nil print-number-table)
     (list (prin1-to-string (vector cell cell))
           (gethash cell print-number-table))))
 '("[#1=(a) #1#]" 1))

(qa
 (progn
   (let ((print-circle t)) (print--preprocess nil))
   (let* ((cell (list 'a))
          (print-circle t)
          (print-continuous-numbering t)
          (print-number-table (make-hash-table :test 'eq)))
     (puthash cell 'first-visit print-number-table)
     (list (prin1-to-string (vector cell cell))
           (gethash cell print-number-table))))
 '("[#1=(a) #1#]" 1))

(defvar qa263-remhash-advice-calls 0)
(defun qa263-remhash-around (original &rest arguments)
  (setq qa263-remhash-advice-calls (1+ qa263-remhash-advice-calls))
  (apply original arguments))
(qa
 (let ((print-circle t)
       (print-continuous-numbering t)
       (print-number-table (make-hash-table :test 'eq))
       (qa263-remhash-advice-calls 0))
   (advice-add 'remhash :around #'qa263-remhash-around)
   (unwind-protect
       (list (prin1-to-string '(a b)) qa263-remhash-advice-calls)
     (advice-remove 'remhash #'qa263-remhash-around)))
 '("(a b)" 0))

(qa
 (progn
   ;; Reset GNU's process-global print_number_index before checking the
   ;; user-supplied equal table.
   (let ((print-circle t)) (print--preprocess nil))
   (let ((print-circle t)
         (print-continuous-numbering t)
         (print-number-table (make-hash-table :test 'equal)))
     (let ((a (copy-sequence "x"))
           (b (copy-sequence "x")))
       (list (prin1-to-string (vector a b))
             (hash-table-count print-number-table)))))
 '("[#1=\"x\" #1#]" 1))

(qa
 (progn
   ;; The print-number index is process-global while continuous numbering
   ;; is active, even when a fresh custom table is supplied.
   (let ((print-circle t)) (print--preprocess nil))
   (define-hash-table-test 'qa263-custom-equal #'equal #'sxhash-equal)
   (let ((print-circle t)
         (print-continuous-numbering t)
         (print-number-table (make-hash-table :test 'qa263-custom-equal)))
     (let ((a (copy-sequence "x"))
           (b (copy-sequence "x")))
       (let ((first (prin1-to-string (vector a b)))
             (second (prin1-to-string (vector a b))))
         (list first second (hash-table-count print-number-table)
               (gethash a print-number-table))))))
 '("[#1=\"x\" #1#]" "[#1# #1#]" 1 1))

;; Number-table state belongs to each hash entry, not to its numeric label:
;; two distinct entries may both define #7= and both become positive after
;; their first print.
(qa
 (let* ((a (list 'a))
        (b (list 'b))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq)))
   (puthash a -7 print-number-table)
   (puthash b -7 print-number-table)
   (let ((first (prin1-to-string (vector a b))))
     (list first
           (gethash a print-number-table)
           (gethash b print-number-table)
           (prin1-to-string (vector a b)))))
 '("[#7=(a) #7=(b)]" 7 7 "[#7# #7#]"))

(qa
 (let ((print-circle t)
       (print-continuous-numbering t)
       (print-number-table (make-hash-table :test 'qa263-custom-equal)))
   (puthash (copy-sequence "x") "replacement" print-number-table)
   (prin1-to-string (copy-sequence "x")))
 "replacement")

;; Empty strings and vectors are excluded by print-circle-candidate-p. A
;; user-supplied number-table entry must not replace either object.
(qa
 (let* ((empty (copy-sequence ""))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq)))
   (puthash empty "replacement" print-number-table)
   (prin1-to-string empty))
 '"\"\"")
(qa
 (let* ((empty [])
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq)))
   (puthash empty "replacement" print-number-table)
   (prin1-to-string empty))
 '"[]")

;; Preprocessing must apply the candidate predicate before a custom hash
;; callback. The root vector is a candidate; its empty children are not.
(qa
 (let ((calls nil))
   (define-hash-table-test 'qa263-print-circle-candidates #'eq
     (lambda (key) (push key calls) 0))
   (let* ((root (vector (copy-sequence "") []))
          (print-circle t)
          (print-number-table
           (make-hash-table :test 'qa263-print-circle-candidates)))
     (print--preprocess root)
     (list (length calls)
           (and (= (length calls) 2)
                (eq (car calls) root)
                (eq (cadr calls) root)))))
 '(2 t))

(qa
 (let* ((object (list 'zero))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq)))
   (puthash object 0 print-number-table)
   (list (prin1-to-string (vector object object))
         (gethash object print-number-table)))
 '("[#0# #0#]" 0))

(defvar qa263-print-table-callback-count 0)
(defun qa263-print-table-callback-once (_object _escape)
  (setq qa263-print-table-callback-count
        (1+ qa263-print-table-callback-count))
  (if (= qa263-print-table-callback-count 1)
      (signal 'file-error '("callback"))
    "later"))
(qa
 (let* ((overlay (make-overlay 1 1))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq))
        (print-unreadable-function #'qa263-print-table-callback-once)
        (qa263-print-table-callback-count 0))
   (puthash overlay -7 print-number-table)
   (let ((first-error
          (condition-case err
              (prin1-to-string (vector overlay overlay))
            (file-error (car err)))))
     (list first-error
           (gethash overlay print-number-table)
           (prin1-to-string (vector overlay overlay))
           (gethash overlay print-number-table)
           qa263-print-table-callback-count)))
 '(file-error -7 "[later later]" -7 3))

(defun qa263-print-table-hash (_object) 0)
(define-hash-table-test 'qa263-print-table-eq #'eq #'qa263-print-table-hash)
(qa
 (let* ((overlay (make-overlay 1 1))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'qa263-print-table-eq))
        (print-unreadable-function #'qa263-print-table-callback-once)
        (qa263-print-table-callback-count 0))
   (puthash overlay -7 print-number-table)
   (let ((first-error
          (condition-case err
              (prin1-to-string (vector overlay overlay))
            (file-error (car err)))))
     (list first-error (gethash overlay print-number-table))))
 '(file-error -7))

(qa
 (let* ((overlay (make-overlay 1 1))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table nil)
        (print-unreadable-function (lambda (_object _escape) "opaque")))
   (list (prin1-to-string (vector overlay overlay))
         (gethash overlay print-number-table)))
 '("[opaque opaque]" nil))

;; With a function output stream GNU consumes a number-table label only
;; after its complete `#N=' prefix has reached the stream.
(qa
 (let* ((object (list 'streamed-label))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq)))
   (puthash object -7 print-number-table)
   (list (condition-case nil
             (prin1 object (lambda (_character) (error "sink failed")))
           (error 'failed))
         (gethash object print-number-table)))
 '(failed -7))

;; Once the complete `#N=' prefix reaches the stream, GNU consumes the table
;; entry before printing the object's body.
(qa
 (let* ((object (list 'streamed-label))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq))
        (count 0)
        (seen nil))
   (puthash object -7 print-number-table)
   (list (condition-case nil
             (prin1 object
                    (lambda (character)
                      (setq count (1+ count))
                      (push (cons character (gethash object print-number-table)) seen)
                      (when (= count 4) (error "sink failed"))))
           (error 'failed))
         (gethash object print-number-table)
         (nreverse seen)))
 '(failed 7 ((35 . -7) (55 . -7) (61 . -7) (40 . 7))))

;; GNU invokes function streams while traversing the object. Mutating the
;; next cons cell from the first emitted character changes the remaining
;; output immediately.
(qa
 (let* ((object (list 'first 'second))
        (characters nil)
        (count 0))
   (prin1 object
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 2)
              (setcar (cdr object) 'changed))))
   (list (apply #'string (nreverse characters))
         (cadr object)))
 '("(first changed)" changed))

(qa
 (let* ((object (vector 'old 'second))
        (count 0)
        (characters nil))
   (prin1 object
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 1)
              (aset object 0 'new))))
   (apply #'string (nreverse characters)))
 "[new second]")

(qa
 (let* ((string (copy-sequence "abc"))
        (count 0)
        (characters nil))
   (princ string
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 1)
              (aset string 1 ?X))))
   (list (apply #'string (nreverse characters)) string))
 '("aXc" "aXc"))

(qa
 (let ((buffer (generate-new-buffer " *qa-print-stream-before*"))
       (characters nil)
       (count 0))
   (unwind-protect
       (progn
         (prin1 buffer
                (lambda (character)
                  (push character characters)
                  (when (= (setq count (1+ count)) 1)
                    (with-current-buffer buffer
                      (rename-buffer "qa-print-stream-after")))))
         (apply #'string (nreverse characters)))
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 "#<buffer qa-print-stream-after>")

(qa
 (let ((marker (make-marker))
       (buffer (generate-new-buffer " *qa-marker-stream-after*"))
       (characters nil)
       (count 0))
   (unwind-protect
       (progn
         (set-marker marker 1 nil)
         (prin1 marker
                (lambda (character)
                  (push character characters)
                  (when (= (setq count (1+ count)) 1)
                    (set-marker marker 1 buffer))))
         (let ((printed (apply #'string (nreverse characters))))
           (and (string-match-p
                 (regexp-quote "in *qa-marker-stream-after*>")
                 printed)
                t)))
     (set-marker marker nil)
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 nil)

;; Function streams do not change the real output's print position.
(qa
 (let ((standard-output (lambda (_character) nil)))
   (princ "x" t)
   (princ "\n")
   (terpri t t))
 t)

(qa
 (let ((print-circle t)
       (print-continuous-numbering nil)
       (print-number-table (make-hash-table :test 'qa263-custom-equal)))
   (prin1-to-string (vector (copy-sequence "x") (copy-sequence "x"))))
 "[\"x\" \"x\"]")

;; Hash pre-scans follow the renderer's key/value order when shared objects
;; occur as both keys and values.
(qa
 (let* ((a (list 'a))
        (b (list 'b))
        (table (make-hash-table :test 'eq)))
   (puthash a a table)
   (puthash b b table)
   (let ((ordinary
          (let ((print-circle t))
            (prin1-to-string table))))
     (let ((print-circle t)
           (print-continuous-numbering t)
           (print-number-table (make-hash-table :test 'eq)))
       (print--preprocess nil)
       (list ordinary (prin1-to-string table)))))
 '("#s(hash-table test eq data (#1=(a) #1# #2=(b) #2#))"
   "#s(hash-table test eq data (#1=(a) #1# #2=(b) #2#))"))

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

;; An unreadable-object callback runs at its position in the object walk. Its
;; nested print can consume a later object's negative number-table entry.
(qa
 (let* ((cell (list 'a))
        (print-circle t)
        (print-continuous-numbering t)
        (print-number-table (make-hash-table :test 'eq))
        (seen nil))
   (let ((outer
          (prin1-to-string
           (vector (current-buffer) cell cell)
           nil
           (list
            (cons 'unreadable-function
                  (lambda (_object _escape)
                    (setq seen
                          (list (prin1-to-string cell)
                                (gethash cell print-number-table)))
                    "opaque"))))))
     (list outer seen (gethash cell print-number-table))))
 '("[1=(a)opaque #1# #1#]" ("#1=(a)" 1) 1))

;; The same nested print must observe an equal-tested number-table entry and
;; leave the outer object as a back-reference after it consumes the label.
(qa
 (progn
   (let ((print-circle t)) (print--preprocess nil))
   (let* ((cell (copy-sequence "x"))
          (print-circle t)
          (print-continuous-numbering t)
          (print-number-table (make-hash-table :test 'equal))
          (seen nil))
     (let ((outer
            (prin1-to-string
             (vector (current-buffer) cell cell)
             nil
             (list
              (cons 'unreadable-function
                    (lambda (_object _escape)
                      (setq seen (prin1-to-string cell))
                      "opaque"))))))
       (list outer seen (gethash cell print-number-table)))))
 '("[1=\"x\"opaque #1# #1#]" "#1=\"x\"" 1))

;; `format' conversions use the active unreadable-object callback too.
(qa
 (let ((print-unreadable-function (lambda (_object _escape) "opaque")))
   (format "%S" (current-buffer)))
 "opaque")
(qa
 (let ((buffer (generate-new-buffer " *qa-format-killed*"))
       (print-unreadable-function (lambda (_object _escape) "killed")))
   (unwind-protect
       (progn
         (kill-buffer buffer)
         (format "%s" buffer))
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 "killed")

;; A non-string, non-nil callback result suppresses the fallback form.
(qa
 (let ((print-unreadable-function
        (lambda (_object _escape) 'handled)))
   (prin1-to-string (vector (current-buffer) 'tail)))
 "[ tail]")

;; `princ' calls the unreadable-object callback before choosing its fallback:
;; live buffers print as names and killed buffers use unreadable syntax.
(qa
 (let* ((buffer (generate-new-buffer " *qa-princ-live*"))
        (callback-arguments nil)
        (print-unreadable-function
         (lambda (object escape)
           (setq callback-arguments (list object escape))
           "live")))
   (unwind-protect
       (list (with-output-to-string (princ buffer))
             (eq (car callback-arguments) buffer)
             (cadr callback-arguments))
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 '("live" t nil))

;; A callback may mutate a later vector slot while that slot is still to be
;; printed. The printer must release the vector borrow before calling Lisp.
(qa
 (let* ((object (vector (current-buffer) 'old))
        (print-unreadable-function
         (lambda (_object _escape)
           (aset object 1 'new)
           "opaque")))
   (prin1-to-string object))
 "[opaque new]")
(qa
 (let* ((object (record 'qa-print-record (current-buffer) 'old))
        (print-unreadable-function
         (lambda (_object _escape)
           (aset object 2 'new)
           "opaque")))
   (prin1-to-string object))
 "#s(qa-print-record opaque new)")

(qa
 (let* ((bits (make-bool-vector 8 nil))
        (characters nil)
        (count 0))
   (prin1 bits
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 1)
              (aset bits 0 1))))
   (let ((printed (apply #'string (nreverse characters))))
     (list (aref printed 4) (aref bits 0))))
 '(1 t))

(qa
 (let* ((subtype (make-symbol "qa-print-stream-char-table"))
        (_ (put subtype 'char-table-extra-slots 1))
        (table (make-char-table subtype))
        (characters nil)
        (count 0)
        (print-length 69))
   (set-char-table-extra-slot table 0 'old)
   (prin1 table
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 1)
              (set-char-table-extra-slot table 0 'new))))
   (let* ((printed (apply #'string (nreverse characters)))
          (suffix (substring printed (- (length printed) 4))))
     (equal suffix "new]")))
 t)

;; A value callback may update a later hash-table entry; print each value
;; after releasing the table borrow and re-read it when its turn arrives.
(qa
 (let* ((table (make-hash-table :test 'eq))
        (print-unreadable-function
         (lambda (_object _escape)
           (puthash 'second 'new table)
           "opaque")))
   (puthash 'first (current-buffer) table)
   (puthash 'second 'old table)
   (prin1-to-string table))
 "#s(hash-table test eq data (first opaque second new))")

(qa
 (let* ((key (current-buffer))
        (table (make-hash-table :test 'eq))
        (print-unreadable-function
         (lambda (_object _escape)
           (puthash key 'new table)
           "opaque")))
   (puthash key 'old table)
   (prin1-to-string table))
 "#s(hash-table test eq data (opaque new))")

(qa
 (let ((table (make-hash-table :test 'eq))
       (print-length 1))
   (puthash 'first 1 table)
   (puthash 'second 2 table)
   (prin1-to-string table))
 "#s(hash-table test eq data (first 1 ...))")

;; The function stream can add entries while the hash-table header is being
;; printed; the printer reads the live entry count after the header.
(qa
 (let* ((table (make-hash-table :test 'eq))
        (characters nil)
        (count 0))
   (prin1 table
          (lambda (character)
            (push character characters)
            (when (= (setq count (1+ count)) 1)
              (puthash 'late 5 table))))
   (apply #'string (nreverse characters)))
 "#s(hash-table test eq data (late 5))")

;; Each unreadable callback sees the print call's original current buffer;
;; printing restores that buffer after both normal returns and errors.
(qa
 (let* ((original (current-buffer))
        (second (generate-new-buffer " *qa-princ-buffer-second*"))
        (target (generate-new-buffer " *qa-princ-buffer-target*"))
        (seen nil)
        (print-unreadable-function
         (lambda (_object _escape)
           (push (current-buffer) seen)
           (set-buffer target)
           "opaque")))
   (unwind-protect
       (let* ((printed (prin1-to-string (vector original second)))
              (seen-original (mapcar (lambda (buffer) (eq buffer original))
                                     (nreverse seen)))
              (restored-after-print (eq (current-buffer) original))
              (error-result
               (let ((print-unreadable-function
                      (lambda (_object _escape)
                        (set-buffer target)
                        (signal 'file-error '("switch")))))
                 (let ((signaled
                        (condition-case error
                            (prin1-to-string original)
                          (file-error (car error)))))
                   (list signaled (eq (current-buffer) original))))))
         (list printed seen-original restored-after-print error-result))
     (dolist (buffer (list second target))
       (when (buffer-live-p buffer) (kill-buffer buffer)))))
 '("[opaque opaque]" (t t) t (file-error t)))

;; Buffer and marker output streams are made current while GNU renders. The
;; unreadable callback observes that destination buffer, then printing
;; restores the caller's current buffer.
(qa
 (let* ((original (current-buffer))
        (target (generate-new-buffer " *qa-printer-buffer-destination*"))
        (seen nil)
        (print-unreadable-function
         (lambda (_object _escape)
           (setq seen (eq (current-buffer) target))
           "opaque")))
   (unwind-protect
       (progn
         (prin1 original target)
         (list seen
               (eq (current-buffer) original)
               (with-current-buffer target (buffer-string))))
     (when (buffer-live-p target)
       (kill-buffer target))))
 '(t t "opaque"))

(qa
 (let* ((original (current-buffer))
        (target (generate-new-buffer " *qa-printer-marker-destination*"))
        (marker (with-current-buffer target (point-marker)))
        (seen nil)
        (print-unreadable-function
         (lambda (_object _escape)
           (setq seen (eq (current-buffer) target))
           "opaque")))
   (unwind-protect
       (progn
         (prin1 original marker)
         (list seen
               (eq (current-buffer) original)
               (with-current-buffer target (buffer-string))
               (marker-position marker)))
     (set-marker marker nil)
     (when (buffer-live-p target)
       (kill-buffer target))))
 '(t t "opaque" 7))

(qa
 (let ((buffer (generate-new-buffer " *qa-princ-killed*"))
       (print-unreadable-function
        (lambda (_object _escape) "killed")))
   (unwind-protect
       (progn
         (kill-buffer buffer)
         (with-output-to-string (princ buffer)))
     (when (buffer-live-p buffer)
       (kill-buffer buffer))))
 "killed")

;; Callback failures unwind the mutable rendering scope, so a later print in
;; the same interpreter still gets fresh circle state.
(qa
 (let* ((cell (list 'a))
        (print-unreadable-function
         (lambda (_object _escape)
           (signal 'file-error '("unreadable callback")))))
   (list
    (condition-case error
        (prin1-to-string (current-buffer))
      (file-error (car error)))
    (let ((print-circle t))
      (prin1-to-string (list cell cell)))))
 '(file-error "(#1=(a) #1#)"))

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

;; A removed hash-table key leaves a tombstone in the insertion-order slots;
;; printing must continue to the later live entries.
(qa
 (let ((table (make-hash-table :test 'eq)))
   (puthash 'first 1 table)
   (puthash 'removed 2 table)
   (puthash 'last 3 table)
   (remhash 'removed table)
   (prin1-to-string table))
 "#s(hash-table test eq data (first 1 last 3))")

;; A key's unreadable callback can remove its hash entry before the printer
;; reaches the value slot; GNU then prints the now-missing value as nil.
(qa
 (let* ((overlay (make-overlay 1 1))
        (table (make-hash-table :test 'eq))
        (print-unreadable-function
         (lambda (object _escape)
           (when (eq object overlay)
             (remhash object table))
           "opaque")))
   (puthash overlay 'old table)
   (prin1-to-string table))
 "#s(hash-table test eq data (opaque nil))")

;; With ENSURE, GNU cannot determine whether a raw lambda-list stream is at
;; beginning-of-line and rejects that unsupported function argument.
(qa-err-data (terpri '(lambda (character) nil) t)
             'error '("Unsupported function argument" lambda
                      (character) nil))
(qa-err-data (terpri '(closure (t) (character) nil) t)
             'invalid-function '((closure (t) (character) nil)))

;; `terpri' must recognize a function-stream lambda by its captured symbol
;; identity after the symbol name leaves the public obarray.
(defvar qa-po-uninterned-stream-events nil)
(qa (let* ((lambda-symbol 'lambda)
           (stream (list lambda-symbol '(character)
                         '(push character qa-po-uninterned-stream-events)))
           (qa-po-uninterned-stream-events nil))
      (unintern lambda-symbol obarray)
      (terpri stream)
      (nreverse qa-po-uninterned-stream-events))
    '(10))

(princ (qa--done)) (terpri)
