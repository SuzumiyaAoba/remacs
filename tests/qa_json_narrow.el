;;; qa_json_narrow.el --- JSON parsing respects accessible bounds -*- lexical-binding: t; -*-

(defun qa302-parse (text begin end position)
  (with-temp-buffer
    (insert text)
    (narrow-to-region begin end)
    (goto-char position)
    (list (condition-case error (json-parse-buffer) (error (car error)))
          (point))))

(qa (qa302-parse "[1,2]" 1 4 1)
    '(json-end-of-file 1))

(qa (qa302-parse "[1,2]" 1 1 1)
    '(json-end-of-file 1))

(qa (qa302-parse "xx[1,2]yy" 3 8 3)
    '([1 2] 8))

(qa (qa302-parse "xx[1,2]yy" 3 6 3)
    '(json-end-of-file 3))

(qa (qa302-parse "xx12yy" 3 5 3)
    '(12 5))

(qa (qa302-parse "xx1234" 3 5 3)
    '(12 5))

(qa (qa302-parse "xxtrue" 3 6 3)
    '(json-parse-error 3))

(qa (qa302-parse "xx\"hi\"zz" 3 6 3)
    '(json-end-of-file 3))

(qa (qa302-parse "xx[1]yy" 3 6 6)
    '(json-end-of-file 6))

(qa (qa302-parse "xx[1] [2]yy" 3 10 7)
    '([2] 10))

(qa (qa302-parse "xxλ [1,2]z" 3 8 5)
    '(json-end-of-file 5))

(qa (with-temp-buffer
      (insert "xx[1] [2]yy")
      (narrow-to-region 3 10)
      (goto-char 3)
      (list (json-parse-buffer) (json-parse-buffer) (point)
            (point-min) (point-max)))
    '([1] [2] 10 3 10))

(princ (qa--done))
