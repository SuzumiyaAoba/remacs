;;; qa_rst_comment_directive.el --- RST comment fontification regression -*- lexical-binding: t; -*-

(provide 'ansi-color)
(provide 'comint)
(provide 'compile)
(require 'rst)

(defun qa-rst-face-list-at (position)
  (let ((face (get-text-property position 'face))
        (font-lock-face (get-text-property position 'font-lock-face)))
    (append (if (listp face) face (list face))
            (if (listp font-lock-face) font-lock-face (list font-lock-face)))))

(defun qa-rst-position-of (text)
  (save-excursion
    (goto-char (point-min))
    (search-forward text)
    (- (point) (length text))))

(qa
 (with-temp-buffer
   (rst-mode)
   (insert ".. .. list-table::\n   :stub-columns: 1\n   :header-rows: 1\n\n..\n   **commented emphasis**\n\nordinary text\n")
   (font-lock-mode 1)
   (font-lock-ensure)
   (let* ((directive-faces (qa-rst-face-list-at
                            (qa-rst-position-of "list-table")))
          (stub-faces (qa-rst-face-list-at
                       (qa-rst-position-of "stub-columns")))
          (header-faces (qa-rst-face-list-at
                         (qa-rst-position-of "header-rows")))
          (empty-comment-faces (qa-rst-face-list-at
                                (qa-rst-position-of "commented emphasis")))
          (ordinary-faces (qa-rst-face-list-at
                           (qa-rst-position-of "ordinary text"))))
     (and (memq 'rst-comment directive-faces)
          (memq 'rst-comment stub-faces)
          (memq 'rst-comment header-faces)
          (not (memq 'rst-directive directive-faces))
          (not (memq 'rst-external stub-faces))
          (not (memq 'rst-external header-faces))
          (memq 'rst-comment empty-comment-faces)
          (not (memq 'rst-emphasis2 empty-comment-faces))
          (not (memq 'rst-comment ordinary-faces)))))
 t)

(princ (qa--done))
