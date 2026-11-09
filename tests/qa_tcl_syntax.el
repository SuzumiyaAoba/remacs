(require 'tcl)

(qa (with-temp-buffer
      (insert "set value {# after}\n")
      (tcl-mode)
      (syntax-propertize (point-max))
      (goto-char (point-min))
      (search-forward "after")
      (nth 4 (syntax-ppss)))
    nil)

(qa (with-temp-buffer
      (insert "# comment\nset value 1\n")
      (tcl-mode)
      (syntax-propertize (point-max))
      (goto-char (point-min))
      (search-forward "comment")
      (nth 4 (syntax-ppss)))
    t)

(qa (with-temp-buffer
      (insert (concat "# comment \\" "\n  continued comment\nputs done\n"))
      (tcl-mode)
      (syntax-propertize (point-max))
      (goto-char (point-min))
      (forward-line 1)
      (let* ((before (current-indentation))
             (calculated (tcl-calculate-indent))
             (result (tcl-indent-line)))
        (list calculated result before (current-indentation)
              (and (nth 4 (syntax-ppss (point))) t))))
    '(t noindent 2 2 t))

(qa (with-temp-buffer
      (insert "set result \"[string length \"abc\"]\"\nputs done\n")
      (tcl-mode)
      (syntax-propertize (point-max))
      (goto-char (point-min))
      (search-forward "abc")
      (and (nth 3 (syntax-ppss)) t))
    t)

(qa (with-temp-buffer
      (insert (concat "# comment " (char-to-string ?\\)
                      "\ncontinued comment\nputs done\n"))
      (tcl-mode)
      (syntax-propertize (point-max))
      (goto-char (point-min))
      (search-forward "continued")
      (nth 4 (syntax-ppss)))
    t)

(princ (qa--done))
