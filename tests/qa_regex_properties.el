;;; -*- lexical-binding: t; -*-
(defun qa276-probe (kind properties pattern property-kind)
 (with-temp-buffer
  (insert "a.a.a")
  (let* ((parse-sexp-lookup-properties properties)
         (entry (if (eq property-kind 'table)
                    (let ((table (copy-syntax-table (syntax-table)))) (modify-syntax-entry ?. "w" table) table)
                  (string-to-syntax (if (eq property-kind 'word) "w" ".")))))
   (put-text-property 2 3 'syntax-table entry)
   (goto-char 1)
   (let ((result (if (eq kind 'string) (string-match pattern (buffer-string))
                   (re-search-forward pattern nil t))))
    (list (not (null result)) (and result (match-beginning 0)) (and result (match-end 0)))))))
(qa
(qa276-probe 'buffer nil "\\w+" 'word)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\W+" 'word)
'(t 2 3)
)

(qa
(qa276-probe 'buffer nil "\\sw+" 'word)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\b." 'word)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "[[:word:]]+" 'word)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\w+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\W+" 'punct)
'(t 2 3)
)

(qa
(qa276-probe 'buffer nil "\\sw+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\b." 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "[[:word:]]+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\w+" 'table)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\W+" 'table)
'(t 2 3)
)

(qa
(qa276-probe 'buffer nil "\\sw+" 'table)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "\\b." 'table)
'(t 1 2)
)

(qa
(qa276-probe 'buffer nil "[[:word:]]+" 'table)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "\\w+" 'word)
'(t 1 4)
)

(qa
(qa276-probe 'buffer t "\\W+" 'word)
'(t 4 5)
)

(qa
(qa276-probe 'buffer t "\\sw+" 'word)
'(t 1 4)
)

(qa
(qa276-probe 'buffer t "\\b." 'word)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "[[:word:]]+" 'word)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "\\w+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "\\W+" 'punct)
'(t 2 3)
)

(qa
(qa276-probe 'buffer t "\\sw+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "\\b." 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "[[:word:]]+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "\\w+" 'table)
'(t 1 4)
)

(qa
(qa276-probe 'buffer t "\\W+" 'table)
'(t 4 5)
)

(qa
(qa276-probe 'buffer t "\\sw+" 'table)
'(t 1 4)
)

(qa
(qa276-probe 'buffer t "\\b." 'table)
'(t 1 2)
)

(qa
(qa276-probe 'buffer t "[[:word:]]+" 'table)
'(t 1 2)
)

(qa
(qa276-probe 'string nil "\\w+" 'word)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\W+" 'word)
'(t 1 2)
)

(qa
(qa276-probe 'string nil "\\sw+" 'word)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\b." 'word)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "[[:word:]]+" 'word)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\w+" 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\W+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'string nil "\\sw+" 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\b." 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "[[:word:]]+" 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\w+" 'table)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\W+" 'table)
'(t 1 2)
)

(qa
(qa276-probe 'string nil "\\sw+" 'table)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "\\b." 'table)
'(t 0 1)
)

(qa
(qa276-probe 'string nil "[[:word:]]+" 'table)
'(t 0 1)
)

(qa
(qa276-probe 'string t "\\w+" 'word)
'(t 0 3)
)

(qa
(qa276-probe 'string t "\\W+" 'word)
'(t 3 4)
)

(qa
(qa276-probe 'string t "\\sw+" 'word)
'(t 0 3)
)

(qa
(qa276-probe 'string t "\\b." 'word)
'(t 0 1)
)

(qa
(qa276-probe 'string t "[[:word:]]+" 'word)
'(t 0 1)
)

(qa
(qa276-probe 'string t "\\w+" 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string t "\\W+" 'punct)
'(t 1 2)
)

(qa
(qa276-probe 'string t "\\sw+" 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string t "\\b." 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string t "[[:word:]]+" 'punct)
'(t 0 1)
)

(qa
(qa276-probe 'string t "\\w+" 'table)
'(t 0 3)
)

(qa
(qa276-probe 'string t "\\W+" 'table)
'(t 3 4)
)

(qa
(qa276-probe 'string t "\\sw+" 'table)
'(t 0 3)
)

(qa
(qa276-probe 'string t "\\b." 'table)
'(t 0 1)
)

(qa
(qa276-probe 'string t "[[:word:]]+" 'table)
'(t 0 1)
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\b")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\B")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\>")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\_<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\_>")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\b")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\B")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\>")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\_<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\_>")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\b")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\B")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\>")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\_<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties nil))
    (goto-char 2) (looking-at "\\_>")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\b")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\B")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\>")))
't
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\_<")))
'nil
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "_"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char 2) (looking-at "\\_>")))
'nil
)

(qa
(with-temp-buffer
  (insert "prefix a.a suffix") (narrow-to-region 8 11)
  (put-text-property 9 10 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char (point-min)) (re-search-forward "\\w+")
    (list (match-beginning 0) (match-end 0))))
'(8 11)
)

(qa
(with-temp-buffer
  (insert "prefix a.a suffix") (narrow-to-region 8 11)
  (put-text-property 9 10 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char (point-max)) (re-search-backward "\\_<\\w+\\_>")
    (list (match-beginning 0) (match-end 0))))
'(8 11)
)

(qa
(with-temp-buffer
  (insert "prefix a.a suffix") (narrow-to-region 8 11)
  (put-text-property 9 10 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (goto-char (point-max))
    (list (looking-back "\\_<\\w+" (point-min) t) (match-beginning 0)
	  (match-end 0))))
'(t 8 11)
)

(qa
(with-temp-buffer
  (insert "a.a")
  (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
  (let ((parse-sexp-lookup-properties t))
    (string-match "\\w+" "a.a")
    (list (match-beginning 0) (match-end 0))))
'(0 1)
)

(qa
(with-temp-buffer
  (let
      ((source (copy-sequence "a.a")) (parse-sexp-lookup-properties t))
    (put-text-property 1 2 'syntax-table (string-to-syntax "w") source)
    (string-match "\\w+" source)
    (list (match-beginning 0) (match-end 0))))
'(0 3)
)

(qa
(with-temp-buffer
  (let
      ((source (copy-sequence "a.a")) (parse-sexp-lookup-properties t))
    (put-text-property 1 2 'syntax-table (string-to-syntax "w") source)
    (string-match "\\`a\\b" source)))
'nil
)

(qa
(with-temp-buffer
  (put 'qa276-category 'syntax-table (string-to-syntax "w"))
  (insert "a.a") (put-text-property 2 3 'category 'qa276-category)
  (let ((parse-sexp-lookup-properties t))
    (goto-char 1) (re-search-forward "\\w+")
    (list (match-beginning 0) (match-end 0))))
'(1 4)
)

(qa
(with-temp-buffer
  (put 'qa276-category 'syntax-table (string-to-syntax "w"))
  (let
      ((source (copy-sequence "a.a")) (parse-sexp-lookup-properties t))
    (put-text-property 1 2 'category 'qa276-category source)
    (string-match "\\w+" source)
    (list (match-beginning 0) (match-end 0))))
'(0 3)
)

(qa
(let
    ((parse-sexp-lookup-properties t)
     (source
      (car
       (read-from-string
	"#(\"a.a.a\" 3 4 (syntax-table (2)) 1 2 (syntax-table (1)))"))))
  (string-match "\\w+" source 2)
  (list (match-beginning 0) (match-end 0)))
'(2 5)
)

(qa
(let
    ((parse-sexp-lookup-properties t)
     (source
      (car
       (read-from-string
	"#(\"a.a.a\" 3 4 (syntax-table (2)) 1 2 (syntax-table (1)))"))))
  (string-match "\\w+" source)
  (list (match-beginning 0) (match-end 0)))
'(0 1)
)


(princ (qa--done)) (terpri)
