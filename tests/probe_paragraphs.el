(defun show () (prin1 (list (buffer-string) (point))) (terpri))
;; forward-paragraph return values + landings
(with-temp-buffer (insert "a\n\nb\n\nc\n") (goto-char 1) (prin1 (list (forward-paragraph 1) (point)))(terpri))
(with-temp-buffer (insert "a\n\nb\n\nc\n") (goto-char 1) (prin1 (list (forward-paragraph 2) (point)))(terpri))
(with-temp-buffer (insert "a\n\nb\n\nc\n") (goto-char 9) (prin1 (list (forward-paragraph -1) (point)))(terpri))
(with-temp-buffer (insert "a\n\nb\n\nc\n") (goto-char 9) (prin1 (list (forward-paragraph -2) (point)))(terpri))
(with-temp-buffer (insert "a\n\nb\n\nc\n") (goto-char 9) (prin1 (list (backward-paragraph 2) (point)))(terpri))
(with-temp-buffer (insert "a\n\nb\n\nc\n") (goto-char 4) (prin1 (list (backward-paragraph) (point)))(terpri))
;; multiple blank lines
(with-temp-buffer (insert "x\n\n\n\ny\n") (goto-char 8) (prin1 (list (forward-paragraph -1) (point)))(terpri))
(with-temp-buffer (insert "x\n\n\n\ny\n") (goto-char 1) (prin1 (list (forward-paragraph 1) (point)))(terpri))
;; indented text + paragraph-start with indentation
(with-temp-buffer (insert "  ind a\n\n  ind b\n") (goto-char 1) (prin1 (list (forward-paragraph 1) (point)))(terpri))
;; sentence motion with double-space semantics
(with-temp-buffer (insert "One.  Two.  Three.") (goto-char 1) (forward-sentence 1) (prin1 (point))(terpri))
(with-temp-buffer (insert "One. Two.  Three.") (goto-char 1) (forward-sentence 1) (prin1 (point))(terpri))
(with-temp-buffer (insert "One.  Two.  Three.") (goto-char 18) (backward-sentence 1) (prin1 (point))(terpri))
(with-temp-buffer (insert "Mr. Smith  went.") (goto-char 1) (forward-sentence 1) (prin1 (point))(terpri))
(with-temp-buffer (insert "A!\nB? C.") (goto-char 1) (forward-sentence 1) (prin1 (point))(terpri) (forward-sentence 1) (prin1 (point))(terpri))
;; sentence-end with paragraph boundary
(with-temp-buffer (insert "No end here\n\nNext para.  Done.") (goto-char 1) (forward-sentence 1) (prin1 (point))(terpri))
;; kill-paragraph across boundaries
(with-temp-buffer (insert "p1 text.\n\np2 more.\n\np3 end.\n") (goto-char 3) (kill-paragraph 2) (show))
(with-temp-buffer (insert "p1 text.\n\np2 more.\n") (goto-char 14) (backward-kill-paragraph 2) (show))
;; mark-end-of-sentence repeat
(with-temp-buffer (insert "S1.  S2.  S3.") (goto-char 1) (mark-end-of-sentence 2) (prin1 (list (point) (mark)))(terpri))
;; count-sentences
(with-temp-buffer (insert "One.  Two.  Three.") (prin1 (count-sentences 1 18))(terpri))
;; transpose-paragraphs / -sentences zero-arg with mark
(with-temp-buffer (insert "p1.\n\np2.\n\np3.\n") (goto-char 2) (transpose-paragraphs 2) (show))
(with-temp-buffer (insert "A1.  B2.  C3.") (goto-char 4) (transpose-sentences 1) (show))
(with-temp-buffer (insert "A1.  B2.  C3.") (goto-char 7) (transpose-sentences -1) (show))
;; kill-sexp error path non-interactive
(with-temp-buffer (insert "(a) b") (goto-char 1) (kill-sexp 1) (show))
(with-temp-buffer (insert "(a) b") (goto-char 6) (backward-kill-sexp) (show))
