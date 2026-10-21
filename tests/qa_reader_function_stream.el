;;; -*- lexical-binding: t; -*-
;; Issue #261: callable streams stop after one object and return lookahead.
(defvar qa261-input nil)
(defvar qa261-events nil)
(defun qa261-source (&optional unread)
 (if unread
  (progn (push (list 'unread unread) qa261-events) (push unread qa261-input))
  (let ((next (pop qa261-input)))
   (push (list 'read next) qa261-events) next)))
(defun qa261-value (value)
 (cond ((stringp value)
        (list 'string (string-to-list value) (multibyte-string-p value)
              (text-properties-at 0 value)))
       ((symbolp value)
        (list 'symbol (string-to-list (symbol-name value))
              (eq value (intern-soft (symbol-name value)))))
       (t value)))
(defun qa261-result (stream)
 (condition-case err (list 'ok (qa261-value (read stream)))
  (error (list 'error (car err) (cdr err)))))
(defun qa261-two (text)
 (let ((qa261-input (string-to-list text)) (qa261-events nil))
  (let* ((first (qa261-result 'qa261-source)) (rest (copy-sequence qa261-input))
         (second (qa261-result 'qa261-source)))
   (list first rest second qa261-input))))
(defun qa261-first-trace (text)
 (let ((qa261-input (string-to-list text)) (qa261-events nil))
  (list (qa261-result 'qa261-source) qa261-input (reverse qa261-events))))

(qa
(qa261-two "(a) (b)")
'((ok (a)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "123 456")
'((ok 123) (32 52 53 54) (ok 456) nil)
)

(qa
(qa261-two "a(b)")
'((ok (symbol (97) t)) (40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "\"hi\"(b)")
'((ok (string (104 105) nil nil)) (40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "'a(b)")
'((ok 'a) (40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "[a](b)")
'((ok [a]) (40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "?a(b)")
'((ok 97) (40 98 41) (ok (b)) nil)
)

(qa
(qa261-two ";comment\n(a)(b)")
'((ok (a)) (40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "#x2a 3")
'((ok 42) (32 51) (ok 3) nil)
)

(qa
(qa261-two "#'(lambda () 1) (b)")
'((ok #'(lambda nil 1)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "(a . b) [c]")
'((ok (a . b)) (32 91 99 93) (ok [c]) nil)
)

(qa
(qa261-two "#(\"a\" 0 1 (face bold)) (b)")
'((ok (string (97) nil (face bold))) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "#s(tag 1 2) (b)")
'((ok #s(tag 1 2)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "#1=(a . #1#) (b)")
'((ok #1=(a . #1#)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "#_a (b)")
'((ok (symbol (97) t)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "#@00ignored")
'((ok (symbol (110 105 108) t)) nil (error end-of-file nil) nil)
)

(qa
(qa261-two "#@5 ignored (b)")
'((ok (b)) nil (error end-of-file nil) nil)
)

(qa
(qa261-two "?\\x41 (b)")
'((ok 65) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "\"\\x41x\" (b)")
'((ok (string (65 120) nil nil)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "?\\101 (b)")
'((ok 65) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "\"\\1x\" (b)")
'((ok (string (1 120) nil nil)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "\"日é\" (b)")
'((ok (string (26085 233) t nil)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "\\a (b)")
'((ok (symbol (97) t)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "(a")
'((error end-of-file nil) nil (error end-of-file nil) nil)
)

(qa
(qa261-two "\"unterminated")
'((error end-of-file nil) nil (error end-of-file nil) nil)
)

(qa
(qa261-two ") (b)")
'((error invalid-read-syntax (")")) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "#z (b)")
'((error invalid-read-syntax ("#z")) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two "(a . b c) (d)")
'((error invalid-read-syntax ("expected )")) (41 32 40 100 41)
  (error invalid-read-syntax (")")) (32 40 100 41))
)

(qa
(qa261-two "")
'((error end-of-file nil) nil (error end-of-file nil) nil)
)

(qa
(qa261-two "  ")
'((error end-of-file nil) nil (error end-of-file nil) nil)
)

(qa
(qa261-first-trace "(a) (b)")
'((ok (a)) (32 40 98 41)
  ((read 40) (read 97) (read 41) (unread 41) (read 41)))
)

(qa
(qa261-first-trace "123 456")
'((ok 123) (32 52 53 54)
  ((read 49) (read 50) (read 51) (read 32) (unread 32)))
)

(qa
(qa261-first-trace "a(b)")
'((ok (symbol (97) t)) (40 98 41) ((read 97) (read 40) (unread 40)))
)

(qa
(qa261-first-trace "\"hi\"(b)")
'((ok (string (104 105) nil nil)) (40 98 41)
  ((read 34) (read 104) (read 105) (read 34)))
)

(qa
(qa261-first-trace "'a(b)")
'((ok 'a) (40 98 41) ((read 39) (read 97) (read 40) (unread 40)))
)

(qa
(qa261-first-trace "[a](b)")
'((ok [a]) (40 98 41)
  ((read 91) (read 97) (read 93) (unread 93) (read 93)))
)

(qa
(qa261-first-trace "?a(b)")
'((ok 97) (40 98 41) ((read 63) (read 97) (read 40) (unread 40)))
)

(qa
(qa261-first-trace "#x2a 3")
'((ok 42) (32 51)
  ((read 35) (read 120) (read 50) (read 97) (read 32) (unread 32)))
)

(qa
(qa261-first-trace "?\\x41 (b)")
'((ok 65) (32 40 98 41)
  ((read 63) (read 92) (read 120) (read 52) (read 49) (read 32)
   (unread 32) (read 32) (unread 32)))
)

(qa
(qa261-first-trace "\"\\x41x\" (b)")
'((ok (string (65 120) nil nil)) (32 40 98 41)
  ((read 34) (read 92) (read 120) (read 52) (read 49) (read 120)
   (unread 120) (read 120) (read 34)))
)

(qa
(qa261-first-trace "?\\1 (b)")
'((ok 1) (32 40 98 41)
  ((read 63) (read 92) (read 49) (read 32) (unread 32) (read 32)
   (unread 32)))
)

(qa
(qa261-first-trace "\"\\1x\" (b)")
'((ok (string (1 120) nil nil)) (32 40 98 41)
  ((read 34) (read 92) (read 49) (read 120) (unread 120) (read 120)
   (read 34)))
)

(qa
(qa261-first-trace ") (b)")
'((error invalid-read-syntax (")")) (32 40 98 41) ((read 41)))
)

(qa
(qa261-first-trace ". (b)")
'((error invalid-read-syntax (".")) (32 40 98 41)
  ((read 46) (read 32) (unread 32)))
)

(qa
(qa261-two (concat (string 0) "(a) (b)"))
'((ok (a)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two (concat (string 8192) "(a)"))
'((ok (symbol (8192) t)) (40 97 41) (ok (a)) nil)
)

(qa
(qa261-two (concat "?" (string 1310720) " (b)"))
'((ok 1310720) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two (concat "\"" (string 1310720) "\" (b)"))
'((ok (string (1310720) t nil)) (32 40 98 41) (ok (b)) nil)
)

(qa
(let ((input (string-to-list "(a) (b)")))
  (let
      ((stream
	(lambda (&optional unread)
	  (if unread (push unread input) (pop input)))))
    (let ((first (read stream))) (list first input (read stream)))))
'((a) (32 40 98 41) (b))
)

(qa
(let ((qa261-input (string-to-list "(a) (b)")) (qa261-events nil))
  (let ((standard-input 'qa261-source)) (list (read) (read nil))))
'((a) (b))
)

(qa
(let ((input (string-to-list "((a) [b c]) (tail)")))
  (let
      ((stream
	(lambda (&optional unread)
	  (if unread (push unread input)
	    (progn
	      (garbage-collect) (read-from-string "(nested)")
	      (pop input))))))
    (list (read stream) (read stream))))
'(((a) [b c]) (tail))
)

(qa
(let ((input (string-to-list "qa261-never-interned-unread 2")))
  (list
   (condition-case err
       (read
	(lambda (&optional unread)
	  (if unread (error "unread failed") (pop input))))
     (error (list (car err) (cdr err))))
   (intern-soft "qa261-never-interned-unread")))
'((error ("unread failed")) nil)
)

(qa
(let ((input (string-to-list "qa261-never-interned-read")))
  (list
   (condition-case err
       (read
	(lambda (&optional unread)
	  (if unread (push unread input)
	    (if input (pop input) (error "read failed")))))
     (error (list (car err) (cdr err))))
   (intern-soft "qa261-never-interned-read")))
'((error ("read failed")) nil)
)

(qa
(let ((input (string-to-list "(a)")))
  (read
   (lambda (&optional unread)
     (if unread (push unread input)
       (if input (pop input) (error "overread"))))))
'(a)
)

(qa
(qa261-result (lambda (&optional unread) nil))
'(error end-of-file nil)
)

(qa
(qa261-result (lambda (&optional unread) -1))
'(error end-of-file nil)
)

(qa
(qa261-result (lambda (&optional unread) 1.5))
'(error end-of-file nil)
)

(qa
(qa261-result (lambda (&optional unread) (expt 2 100)))
'(error end-of-file nil)
)

(qa
(qa261-result (lambda (&optional unread) [1]))
'(error end-of-file nil)
)

(qa
(qa261-result (lambda (&optional unread) 4194304))
'(error end-of-file nil)
)

(qa
(qa261-first-trace (concat "123" (string 0) "456"))
'((ok 123) (0 52 53 54)
  ((read 49) (read 50) (read 51) (read 0) (unread 0)))
)

(qa
(qa261-two (concat "123" (string 0) "456"))
'((ok 123) (0 52 53 54) (ok 456) nil)
)

(qa
(qa261-first-trace "?aa (b)")
'((error invalid-read-syntax ("?")) (97 32 40 98 41)
  ((read 63) (read 97) (read 97) (unread 97)))
)

(qa
(qa261-two "?aa (b)")
'((error invalid-read-syntax ("?")) (97 32 40 98 41)
  (ok (symbol (97) t)) (32 40 98 41))
)

(qa
(qa261-first-trace "?\\sx (b)")
'((error invalid-read-syntax ("?")) (120 32 40 98 41)
  ((read 63) (read 92) (read 115) (read 120) (unread 120) (read 120)
   (unread 120)))
)

(qa
(qa261-first-trace "\"\\sx\" (b)")
'((ok (string (32 120) nil nil)) (32 40 98 41)
  ((read 34) (read 92) (read 115) (read 120) (read 34)))
)

(qa
(qa261-result
 (lambda (&optional unread) (signal 'end-of-file '(provider-eof))))
'(error end-of-file (provider-eof))
)

(qa
(let ((input (string-to-list "qa261-eof-on-unread ")))
  (qa261-result
   (lambda (&optional unread)
     (if unread (signal 'end-of-file '(unread-eof)) (pop input)))))
'(error end-of-file (unread-eof))
)

(qa
(catch 'qa261-stop
  (read (lambda (&optional unread) (throw 'qa261-stop 'stopped))))
'stopped
)

(qa
(let ((input (string-to-list "abc ")))
  (list
   (catch 'qa261-stop
     (read
      (lambda (&optional unread)
	(if unread (throw 'qa261-stop unread) (pop input)))))
   input))
'(32 nil)
)

(qa
(qa261-two "\"\\s-a\" (b)")
'((ok (string (32 45 97) nil nil)) (32 40 98 41) (ok (b)) nil)
)

(qa
(let ((input (string-to-list "#1=(a . #1#) (tail)")))
  (let
      ((stream
	(lambda (&optional unread)
	  (if unread (push unread input)
	    (progn (garbage-collect) (pop input))))))
    (list (read stream) (read stream))))
'(#1=(a . #1#) (tail))
)

(qa
(qa261-two "\"\\x140000\" (b)")
'((ok (string (1310720) t nil)) (32 40 98 41) (ok (b)) nil)
)

(qa
(qa261-two (concat (string 1310720) " (b)"))
'((ok (symbol (1310720) t)) (32 40 98 41) (ok (b)) nil)
)


(princ (qa--done)) (terpri)
