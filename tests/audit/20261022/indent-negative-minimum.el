(prin1 (with-temp-buffer (insert "abc") (let ((result (indent-to 0 -1))) (list result (point) (current-column) (buffer-string) (progn (goto-char (point)) (current-column))))))
