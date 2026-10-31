;;; -*- lexical-binding: t -*-
;; Pin the closed parser form and the following top-level definitions (#1786).
(require 'comint)
(load (expand-file-name "../lisp/pcomplete.el"
                        (file-name-directory load-file-name)) nil t)

(qa (featurep 'pcomplete) t)
(qa (fboundp 'pcomplete-parse-comint-arguments) t)
(qa (fboundp 'pcomplete-parse-arguments) t)
(qa (fboundp 'pcomplete-entries) t)

(defun qa-pcomplete--parse-line (line)
  (with-temp-buffer
    (insert line)
    (let* ((position (point))
           (result (pcomplete-parse-comint-arguments)))
      (list (car result) (cdr result) (= position (point))))))

(qa (qa-pcomplete--parse-line "one two") '(("one" "two") (1 5) t))
(qa (qa-pcomplete--parse-line "cmd \"a b\" 'c d' e\\ f")
    '(("cmd" "\"a b\"" "'c d'" "e\\ f") (1 5 11 17) t))
(qa (qa-pcomplete--parse-line "cmd \"a b") '(("cmd" "\"a b") (1 5) t))
(qa (qa-pcomplete--parse-line "cmd  ") '(("cmd" "") (1 6) t))
(qa (qa-pcomplete--parse-line "") '(nil nil t))

(princ (qa--done))
