;;; -*- lexical-binding: nil; -*-
;; Issue #96: START defaults, bounds, and GNU CHECK_FIXNUM validation.
(qa (string-search "A" "abc" nil) nil)
(qa (string-search "a" "abc" nil) 0)
(qa (string-search "" "abc" nil) 0)
(qa (string-search "" "" nil) 0)
(qa (string-search "a" "" nil) nil)
(qa (string-search "" "abc" 3) 3)
(qa (string-search "a" "abc" 3) nil)

(dolist (needle '("" "a" "longer-than-the-haystack"))
  (dolist (start (list -1 4 most-positive-fixnum most-negative-fixnum))
    (qa-err-data (string-search needle "abc" start)
                 'args-out-of-range (list start))))
(qa-err-data (string-search "" "" 1) 'args-out-of-range '(1))
(qa-err-data (string-search "a" "" -1) 'args-out-of-range '(-1))
(dolist (start (list t 'start "0" 0.0 1.0 0.0e+NaN 1.0e+INF
                    (1+ most-positive-fixnum) (1- most-negative-fixnum)
                    170141183460469231731687303715884105727
                    -170141183460469231731687303715884105728))
  (qa-err-data (string-search "" "abc" start)
               'wrong-type-argument (list 'fixnump start)))
(qa-err-data (string-search "a" "abc" (make-marker))
             'wrong-type-argument (list 'fixnump (make-marker)))
(qa-err-data (string-search nil "abc" -1) 'wrong-type-argument '(stringp nil))
(qa-err-data (string-search "a" nil -1) 'wrong-type-argument '(stringp nil))

;; Offsets count characters, including at the end of a multibyte string.
(let ((expected '(1 1 4 4 4 nil nil)))
  (dotimes (start 7)
    (qa (string-search "日" "a日本a日本" start) (nth start expected))
    (qa (string-search "" "a日本a日本" start) start)))
(qa-err-data (string-search "日" "a日本a日本" 7) 'args-out-of-range '(7))
(qa (string-search (unibyte-string 255) (unibyte-string 255 97 255) 1) 2)
(qa (string-search "b" (propertize "abc" 'face 'bold) nil) 1)
(qa (let ((case-fold-search t)) (string-search "A" "abc" nil)) nil)
(qa (progn (string-match "b" "abc")
           (string-search "c" "abc" nil)
           (match-data)) '(1 2))
(qa-err (string-search "a") 'wrong-number-of-arguments)
(qa-err (string-search "a" "abc" 0 nil) 'wrong-number-of-arguments)

(princ (qa--done))
