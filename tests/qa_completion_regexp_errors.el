;;; qa_completion_regexp_errors.el --- Propagate completion filter errors -*- lexical-binding: t; -*-

(defun qa321-filter (operation regexps kind &optional input)
  (let* ((completion-regexp-list regexps)
         (names '("qa321-foo" "qa321-foobar"))
         (table (cond ((eq kind 'hash)
                       (let ((hash (make-hash-table :test 'equal)))
                         (dolist (name names) (puthash name t hash)) hash))
                      ((eq kind 'obarray)
                       (let ((array (make-vector 7 0)))
                         (dolist (name names) (intern name array)) array))
                      ((eq kind 'default)
                       (dolist (name names) (intern name)) obarray)
                      (t names))))
    (unwind-protect
        (condition-case error
            (let ((result (funcall operation (or input "qa321-foo") table)))
              (if (and (eq operation 'all-completions) (consp result))
                  (sort result #'string<) result))
          (error error))
      (when (eq kind 'default) (dolist (name names) (unintern name obarray))))))

;; try-completion validates list filters
(qa (qa321-filter #'try-completion '("[") 'list)
    '(invalid-regexp "Unmatched [ or [^"))

;; try-completion validates list filters
(qa (qa321-filter #'try-completion '(42) 'list)
    '(wrong-type-argument stringp 42))

;; try-completion validates hash filters
(qa (qa321-filter #'try-completion '("[") 'hash)
    '(invalid-regexp "Unmatched [ or [^"))

;; try-completion validates hash filters
(qa (qa321-filter #'try-completion '(42) 'hash)
    '(wrong-type-argument stringp 42))

;; try-completion validates obarray filters
(qa (qa321-filter #'try-completion '("[") 'obarray)
    '(invalid-regexp "Unmatched [ or [^"))

;; try-completion validates obarray filters
(qa (qa321-filter #'try-completion '(42) 'obarray)
    '(wrong-type-argument stringp 42))

;; try-completion validates default filters
(qa (qa321-filter #'try-completion '("[") 'default)
    '(invalid-regexp "Unmatched [ or [^"))

;; try-completion validates default filters
(qa (qa321-filter #'try-completion '(42) 'default)
    '(wrong-type-argument stringp 42))

;; try-completion matching filter
(qa (qa321-filter #'try-completion '("foo$") 'list nil)
    't)

;; try-completion filter selects a longer candidate
(qa (qa321-filter #'try-completion '("bar$") 'list nil)
    '"qa321-foobar")

;; try-completion short circuit before invalid later regexp
(qa (qa321-filter #'try-completion '("nomatch" "[") 'list nil)
    'nil)

;; try-completion no matching candidates means no regexp evaluation
(qa (qa321-filter #'try-completion '("[") 'list "absent")
    'nil)

;; try-completion non-cons filter list is ignored
(qa (qa321-filter #'try-completion 42 'list nil)
    '"qa321-foo")

;; try-completion dotted tail is ignored
(qa (qa321-filter #'try-completion '("foo" . 42) 'list nil)
    '"qa321-foo")

;; try-completion function table owns filtering
(qa (let ((completion-regexp-list '("[" 42))) (funcall #'try-completion "foo" (lambda (&rest _) 'callback-result)))
    'callback-result)

;; all-completions validates list filters
(qa (qa321-filter #'all-completions '("[") 'list)
    '(invalid-regexp "Unmatched [ or [^"))

;; all-completions validates list filters
(qa (qa321-filter #'all-completions '(42) 'list)
    '(wrong-type-argument stringp 42))

;; all-completions validates hash filters
(qa (qa321-filter #'all-completions '("[") 'hash)
    '(invalid-regexp "Unmatched [ or [^"))

;; all-completions validates hash filters
(qa (qa321-filter #'all-completions '(42) 'hash)
    '(wrong-type-argument stringp 42))

;; all-completions validates obarray filters
(qa (qa321-filter #'all-completions '("[") 'obarray)
    '(invalid-regexp "Unmatched [ or [^"))

;; all-completions validates obarray filters
(qa (qa321-filter #'all-completions '(42) 'obarray)
    '(wrong-type-argument stringp 42))

;; all-completions validates default filters
(qa (qa321-filter #'all-completions '("[") 'default)
    '(invalid-regexp "Unmatched [ or [^"))

;; all-completions validates default filters
(qa (qa321-filter #'all-completions '(42) 'default)
    '(wrong-type-argument stringp 42))

;; all-completions matching filter
(qa (qa321-filter #'all-completions '("foo$") 'list nil)
    '("qa321-foo"))

;; all-completions filter selects a longer candidate
(qa (qa321-filter #'all-completions '("bar$") 'list nil)
    '("qa321-foobar"))

;; all-completions short circuit before invalid later regexp
(qa (qa321-filter #'all-completions '("nomatch" "[") 'list nil)
    'nil)

;; all-completions no matching candidates means no regexp evaluation
(qa (qa321-filter #'all-completions '("[") 'list "absent")
    'nil)

;; all-completions non-cons filter list is ignored
(qa (qa321-filter #'all-completions 42 'list nil)
    '("qa321-foo" "qa321-foobar"))

;; all-completions dotted tail is ignored
(qa (qa321-filter #'all-completions '("foo" . 42) 'list nil)
    '("qa321-foo" "qa321-foobar"))

;; all-completions function table owns filtering
(qa (let ((completion-regexp-list '("[" 42))) (funcall #'all-completions "foo" (lambda (&rest _) 'callback-result)))
    'callback-result)

;; test-completion validates list filters
(qa (qa321-filter #'test-completion '("[") 'list)
    '(invalid-regexp "Unmatched [ or [^"))

;; test-completion validates list filters
(qa (qa321-filter #'test-completion '(42) 'list)
    '(wrong-type-argument stringp 42))

;; test-completion validates hash filters
(qa (qa321-filter #'test-completion '("[") 'hash)
    '(invalid-regexp "Unmatched [ or [^"))

;; test-completion validates hash filters
(qa (qa321-filter #'test-completion '(42) 'hash)
    '(wrong-type-argument stringp 42))

;; test-completion validates obarray filters
(qa (qa321-filter #'test-completion '("[") 'obarray)
    '(invalid-regexp "Unmatched [ or [^"))

;; test-completion validates obarray filters
(qa (qa321-filter #'test-completion '(42) 'obarray)
    '(wrong-type-argument stringp 42))

;; test-completion validates default filters
(qa (qa321-filter #'test-completion '("[") 'default)
    '(invalid-regexp "Unmatched [ or [^"))

;; test-completion validates default filters
(qa (qa321-filter #'test-completion '(42) 'default)
    '(wrong-type-argument stringp 42))

;; test-completion matching filter
(qa (qa321-filter #'test-completion '("foo$") 'list nil)
    't)

;; test-completion filter selects a longer candidate
(qa (qa321-filter #'test-completion '("bar$") 'list nil)
    'nil)

;; test-completion short circuit before invalid later regexp
(qa (qa321-filter #'test-completion '("nomatch" "[") 'list nil)
    'nil)

;; test-completion no matching candidates means no regexp evaluation
(qa (qa321-filter #'test-completion '("[") 'list "absent")
    'nil)

;; test-completion non-cons filter list is ignored
(qa (qa321-filter #'test-completion 42 'list nil)
    't)

;; test-completion dotted tail is ignored
(qa (qa321-filter #'test-completion '("foo" . 42) 'list nil)
    't)

;; test-completion function table owns filtering
(qa (let ((completion-regexp-list '("[" 42))) (funcall #'test-completion "foo" (lambda (&rest _) 'callback-result)))
    'callback-result)

;; completion filtering preserves match data
(qa (progn (string-match "a" "abc") (let ((before (match-data))) (qa321-filter #'all-completions '("foo") 'list) (equal before (match-data))))
    't)

(princ (qa--done))
