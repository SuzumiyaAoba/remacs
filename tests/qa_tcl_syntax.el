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
      (insert "set result \"[string length \"abc\"]\"\nputs done\n")
      (tcl-mode)
      (syntax-propertize (point-max))
      (goto-char (point-min))
      (search-forward "abc")
      (and (nth 3 (syntax-ppss)) t))
    t)

(princ (qa--done))
