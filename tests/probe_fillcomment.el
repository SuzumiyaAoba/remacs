(defun show () (prin1 (list (buffer-string) (point))) (terpri))
;; fill-paragraph basic
(with-temp-buffer
  (setq fill-column 20)
  (insert "aaa bbb ccc ddd eee fff ggg")
  (fill-paragraph) (show))
;; fill-region-as-paragraph with indent
(with-temp-buffer
  (setq fill-column 20)
  (insert "  aa bb cc dd ee ff gg hh ii")
  (fill-region-as-paragraph (point-min) (point-max)) (show))
;; fill-region over 2 paragraphs
(with-temp-buffer
  (setq fill-column 15)
  (insert "one two three four.\n\nfive six seven eight.")
  (fill-region (point-min) (point-max)) (show))
;; justify full
(with-temp-buffer
  (setq fill-column 30)
  (insert "aa bb cc dd ee")
  (let ((fill-column 30)) (fill-region-as-paragraph 1 15 'full)) (show))
;; justify right / center via justify-current-line
(with-temp-buffer
  (insert "aa bb cc")
  (let ((fill-column 20)) (justify-current-line 'right)) (show))
(with-temp-buffer
  (insert "aa bb cc")
  (let ((fill-column 20)) (justify-current-line 'center)) (show))
;; canonically-space-region
(with-temp-buffer
  (insert "a   b.    c    d")
  (canonically-space-region 1 16) (show))
;; comment-region lisp-style
(with-temp-buffer
  (setq comment-start ";;" comment-end "")
  (insert "aa\nbb\ncc\n")
  (comment-region 1 9) (show))
;; uncomment-region roundtrip
(with-temp-buffer
  (setq comment-start "##" comment-end "")
  (insert "aa\nbb\n")
  (comment-region 1 5)
  (uncomment-region 1 (point-max)) (show))
;; comment-region C-style block
(with-temp-buffer
  (setq comment-start "/* " comment-end " */")
  (insert "x\ny\n")
  (comment-region 1 5) (show))
;; comment-box
(with-temp-buffer
  (setq comment-start "/* " comment-end " */" comment-style 'box)
  (insert "aa bb")
  (comment-box 1 6 1) (show))
;; comment-indent at comment-column
(with-temp-buffer
  (setq comment-start ";;" comment-end "")
  (insert "code")
  (let ((comment-column 10)) (comment-indent)) (show))
;; adaptive fill prefix
(with-temp-buffer
  (setq fill-column 25)
  (insert "> aa bb cc dd ee ff gg hh ii jj")
  (fill-paragraph) (show))
;; fill-prefix var
(with-temp-buffer
  (setq fill-column 20 fill-prefix "* ")
  (insert "* aa bb cc dd ee")
  (fill-paragraph) (show))
;; fill-nonuniform-paragraphs
(with-temp-buffer
  (setq fill-column 20)
  (insert "aa bb cc dd ee ff\n  gg hh ii jj kk ll\n")
  (fill-nonuniform-paragraphs 1 (point-max)) (show))
;; unfill-paragraph
(with-temp-buffer
  (insert "aa bb\ncc dd\n\n")
  (unfill-paragraph 1) (show))
;; comment-line
(with-temp-buffer
  (setq comment-start "#" comment-end "")
  (insert "x\ny\n")
  (goto-char 1)
  (comment-line 1) (show))
;; comment-dwim on blank line
(with-temp-buffer
  (setq comment-start "//" comment-end "")
  (comment-dwim nil) (show))
;; uncomment-region with arg
(with-temp-buffer
  (setq comment-start ";;" comment-end "")
  (insert ";;;; aa\n;;;; bb\n")
  (uncomment-region 1 (point-max) 2) (show))
;; comment-kill
(with-temp-buffer
  (setq comment-start ";;" comment-end "")
  (insert "code ;; note\nnext ;; more\n")
  (goto-char 1)
  (comment-kill 1) (show))
;; set-fill-prefix
(with-temp-buffer
  (insert "> quoted\n> more\n")
  (goto-char 1)
  (set-fill-prefix) (prin1 (list 'fp fill-prefix))(terpri))
;; fill-individual-paragraphs
(with-temp-buffer
  (setq fill-column 15)
  (insert "> a b c d\n> e f g\n\nh i j k l\n")
  (fill-individual-paragraphs 1 (point-max)) (show))
;; comment-or-uncomment-region on comments
(with-temp-buffer
  (setq comment-start "#" comment-end "")
  (insert "# a\n# b\n")
  (comment-or-uncomment-region 1 8) (show))
;; comment-only-p
(with-temp-buffer
  (setq comment-start "#" comment-end "")
  (insert "# a\n# b\n")
  (prin1 (comment-only-p 1 8))(terpri))
;; fill-forward-paragraph
(with-temp-buffer
  (setq fill-column 15)
  (insert "a b c d e f\n\ng h i j k\n")
  (goto-char 1)
  (fill-forward-paragraph 1) (show))
;; uncomment-region-default roundtrip with arg on block
(with-temp-buffer
  (setq comment-start "/* " comment-end " */")
  (insert "x\ny\n")
  (comment-region 1 5)
  (uncomment-region 1 (point-max)) (show))
