;;; qa_peg_translate_linear.el --- PEG translation size regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/peg.el"
       (file-name-directory load-file-name)))

(defun qa-peg-failure (text form)
  "Return the failure data for FORM on TEXT using the current PEG source."
  (with-temp-buffer
    (insert text)
    (goto-char (point-min))
    (condition-case err
        (eval form t)
      (peg-search-failed
       (list (car err) (cadr err)
             (sort (copy-sequence (nth 2 err)) #'string-lessp)
             (point))))))

(qa
 (list
  (qa-peg-failure "ax" '(peg-parse "a" "b"))
  (qa-peg-failure "xx" '(peg-parse "a" "b"))
  (qa-peg-failure "abx" '(peg-parse "a" "b" "c"))
  (qa-peg-failure "ax" '(peg-parse (or (and "a" "b") (and "a" "c"))))
  (qa-peg-failure "abx" '(peg-parse (or (and "a" "b" "c") (and "a" "d"))))
  (qa-peg-failure "x" '(peg-parse (+ "a")))
  (qa-peg-failure "x" '(peg-parse (guard t) "b"))
  (qa-peg-failure "日x" '(peg-parse "日" "本")))
 '((peg-search-failed 2 ("b") 2)
   (peg-search-failed 1 ("a") 1)
   (peg-search-failed 3 ("c") 3)
   (peg-search-failed 2 ("b" "c") 2)
   (peg-search-failed 3 ("c") 3)
   (peg-search-failed 1 ("a") 1)
   (peg-search-failed 1 ("b") 1)
   (peg-search-failed 2 ("本") 2)))

(defun qa-peg-translation-size (depth)
  "Return the printed size of a translated PEG with DEPTH sequence nodes."
  (let ((exp '(str "x")))
    (dotimes (_ depth)
      (setq exp (list 'and exp '(str "y"))))
    (length (prin1-to-string (peg-translate-exp exp)))))

(qa
 (< (qa-peg-translation-size 24)
    (* 3 (qa-peg-translation-size 12)))
 t)

(princ (qa--done))
