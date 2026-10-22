;;; qa_json_number_range.el --- JSON float overflow checks -*- lexical-binding: t; -*-
;; Overflow is an error, while finite limits, underflow and bignums remain valid.

;; 1e9999
(qa (condition-case error (json-parse-string "1e9999") (error error))
    '(json-number-out-of-range-error 1 nil 6))

;; -1e9999
(qa (condition-case error (json-parse-string "-1e9999") (error error))
    '(json-number-out-of-range-error 1 nil 7))

;; 1e309
(qa (condition-case error (json-parse-string "1e309") (error error))
    '(json-number-out-of-range-error 1 nil 5))

;; -1e309
(qa (condition-case error (json-parse-string "-1e309") (error error))
    '(json-number-out-of-range-error 1 nil 6))

;; 1.7976931348623159e308
(qa (condition-case error (json-parse-string "1.7976931348623159e308") (error error))
    '(json-number-out-of-range-error 1 nil 22))

;; -1.7976931348623159e308
(qa (condition-case error (json-parse-string "-1.7976931348623159e308") (error error))
    '(json-number-out-of-range-error 1 nil 23))

;; 1e9999x
(qa (condition-case error (json-parse-string "1e9999x") (error error))
    '(json-number-out-of-range-error 1 nil 6))

;; newline 1e9999
(qa (condition-case error (json-parse-string "\n1e9999") (error error))
    '(json-number-out-of-range-error 2 nil 7))

;; [1,1e9999]
(qa (condition-case error (json-parse-string "[1,1e9999]") (error error))
    '(json-number-out-of-range-error 1 nil 9))

;; {"é":1e9999}
(qa (condition-case error (json-parse-string "{\"é\":1e9999}") (error error))
    '(json-number-out-of-range-error 1 nil 11))

;; 1.7976931348623157e308
(qa (condition-case error (json-parse-string "1.7976931348623157e308") (error error))
    '1.7976931348623157e+308)

;; 1.7976931348623158e308
(qa (condition-case error (json-parse-string "1.7976931348623158e308") (error error))
    '1.7976931348623157e+308)

;; 1e308
(qa (condition-case error (json-parse-string "1e308") (error error))
    '1e+308)

;; 1e-9999
(qa (condition-case error (json-parse-string "1e-9999") (error error))
    '0.0)

;; -1e-9999
(qa (condition-case error (json-parse-string "-1e-9999") (error error))
    '-0.0)

;; 0e9999
(qa (condition-case error (json-parse-string "0e9999") (error error))
    '0.0)

;; -0e9999
(qa (condition-case error (json-parse-string "-0e9999") (error error))
    '-0.0)

;; 1e+
(qa (condition-case error (json-parse-string "1e+") (error error))
    '(json-end-of-file 1 nil 3))

;; negative zero sign
(qa (condition-case error (prin1-to-string (json-parse-string "-1e-9999")) (error error))
    '"-0.0")

;; large integer remains integer
(qa (condition-case error (integerp (json-parse-string (make-string 400 ?9))) (error error))
    't)

;; unibyte UTF-8 error offset
(qa (condition-case error (json-parse-string (encode-coding-string "{\"é\":1e9999}" 'utf-8)) (error error))
    '(json-number-out-of-range-error 1 nil 12))

;; buffer overflow
(qa (condition-case error (with-temp-buffer (insert "1e9999 tail") (goto-char 1) (list (condition-case error (json-parse-buffer) (error error)) (point))) (error error))
    '((json-number-out-of-range-error 1 nil 6) 1))

;; buffer offset
(qa (condition-case error (with-temp-buffer (insert "zz 1e9999 tail") (goto-char 3) (list (condition-case error (json-parse-buffer) (error error)) (point))) (error error))
    '((json-number-out-of-range-error 1 nil 7) 3))

;; narrowed buffer overflow
(qa (condition-case error (with-temp-buffer (insert "zz1e9999yy") (narrow-to-region 3 9) (goto-char 3) (list (condition-case error (json-parse-buffer) (error error)) (point))) (error error))
    '((json-number-out-of-range-error 1 nil 6) 3))

;; error hierarchy
(qa (condition-case error (condition-case nil (json-parse-string "1e9999") (json-parse-error 'wrong-parent) (json-error 'json-error)) (error error))
    'json-error)

(princ (qa--done))
